//! The Place-pair copy MATERIALIZER (codegen cleanup Phase 6, the Copy*
//! pilot): one routine that walks a source and a target [`Place`] and emits
//! the address computation plus the chunked copy that the per-variant Copy
//! encoders each hand-spelled. Register discipline matches those encoders
//! exactly -- r14 carries the source address, r15 the target address, rax is
//! the chunk scratch -- and a materialized place emits BYTE-FOR-BYTE what
//! the corresponding retired encoder emitted, so the relocation walker's
//! per-kind byte math is unchanged while variants delegate here one by one.
//!
//! Base note: each place's base is `mov rXX, imm64(0)`; WHICH region that
//! placeholder relocates to still lives on the operation kind and is patched
//! by the instruction-record walker (omega-relocations). The walker adopts
//! the place's own region when the kinds themselves collapse -- until then
//! `Place::region` is documentation, not behavior, on this path.
//!
//! Index discipline (the ScaledIndex rung, extended by the double-index
//! rung to TWO): a place may carry AT MOST TWO
//! runtime scaled index, its slot readable from the place's own base region
//! -- the index loads into r11 at its declared width, zero-extended, and scales IMMEDIATELY
//! AFTER the base materializes and BEFORE any deref consumes the base, then
//! `add reg, r11` fires at the step's position in the walk. On the
//! shared-base path an index is legal only on a side that DEREFS (a direct
//! side's add would mutate the base the other side still needs), and only
//! one side may be indexed (r11 is the single index scratch). Everything
//! else REFUSES LOUDLY -- legalization, not silent truncation.

use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};
use omega_target_operations::{
    Place, PlaceStep, RuntimeStorageRegion, RuntimeValueOperandHandle, RuntimeValueOperandSource,
    StateGuardOperator,
};
use psi_diagnostics::Diagnostic;
use psi_numerics::arithmetic::ArithmeticDomain;

/// Which side of the copy a base-materialization relocation site belongs to.
/// The relocation walker maps a side to that place's own region -- this is
/// how `CopyPlaces` patches BY PLACE REGION instead of by per-kind offset
/// functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaceCopySide {
    #[default]
    Source,
    Target,
    /// The source place's ScaledIndex slot base (cross-region index): the
    /// walker patches it from the step's own `index_region`.
    SourceIndex,
    /// The target place's ScaledIndex slot base.
    TargetIndex,
    /// The source place's SECOND ScaledIndex slot base (the double-index
    /// rung; r10 is the second index scratch).
    SourceIndex2,
    /// The target place's SECOND ScaledIndex slot base.
    TargetIndex2,
}

/// Six covers every emitted shape: two bases, plus up to two cross-region
/// index-base materializations per side (the double-index rung).
pub const PLACE_COPY_MAX_SITES: usize = 6;

/// The base-materialization relocation sites of one place copy: the byte
/// position of each `mov r??, imm64(0)` placeholder WITHIN the encoded
/// instruction, tagged with the side whose region patches it. Recorded by
/// the SAME walk that emits the bytes -- lockstep by construction, never a
/// hand-maintained offset constant.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlaceCopySites {
    sites: [(u32, PlaceCopySide); PLACE_COPY_MAX_SITES],
    len: u8,
}

impl PlaceCopySites {
    fn record(&mut self, byte_offset: usize, side: PlaceCopySide) {
        debug_assert!(usize::from(self.len) < PLACE_COPY_MAX_SITES);
        if usize::from(self.len) < PLACE_COPY_MAX_SITES {
            self.sites[usize::from(self.len)] = (byte_offset as u32, side);
            self.len += 1;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, PlaceCopySide)> + '_ {
        self.sites[..usize::from(self.len)]
            .iter()
            .map(|(offset, side)| (*offset as usize, *side))
    }
}

#[derive(Clone, Copy)]
enum AddressRegister {
    /// The source-address register (r14).
    Source,
    /// The target-address register (r15).
    Target,
}

impl AddressRegister {
    fn side(self) -> PlaceCopySide {
        match self {
            AddressRegister::Source => PlaceCopySide::Source,
            AddressRegister::Target => PlaceCopySide::Target,
        }
    }
}

/// Emit the address computation for `place` into the chosen register and
/// return the RESIDUAL displacement: the trailing run of constant offsets is
/// folded into the subsequent load/store displacements instead of being
/// added to the register, exactly as the retired per-variant encoders did.
fn materialize_place_address(
    bytes: &mut Vec<u8>,
    sites: &mut PlaceCopySites,
    place: &Place,
    register: AddressRegister,
) -> Result<usize, Diagnostic> {
    sites.record(bytes.len(), register.side());
    match register {
        AddressRegister::Source => super::append_mov_r14_imm64(bytes, 0),
        AddressRegister::Target => super::append_mov_r15_imm64(bytes, 0),
    }
    // The index (at most one) loads and scales BEFORE any deref consumes the
    // base register its slot is addressed from.
    let index_sides = match register {
        AddressRegister::Source => (PlaceCopySide::SourceIndex, PlaceCopySide::SourceIndex2),
        AddressRegister::Target => (PlaceCopySide::TargetIndex, PlaceCopySide::TargetIndex2),
    };
    prepare_place_index(bytes, sites, place, register, index_sides)?;
    let mut displacement = 0usize;
    let mut index_ordinal = 0usize;
    for step in place.steps() {
        match step {
            PlaceStep::ConstOffset(offset) => displacement += offset,
            PlaceStep::Deref => {
                match register {
                    AddressRegister::Source => {
                        super::append_load_r14_from_r14(bytes, displacement)?
                    }
                    AddressRegister::Target => {
                        super::append_load_r15_from_r15(bytes, displacement)?
                    }
                }
                displacement = 0;
            }
            PlaceStep::ScaledIndex { .. } => {
                append_scaled_index_add(bytes, register, index_ordinal);
                index_ordinal += 1;
            }
        }
    }
    Ok(displacement)
}

/// Pre-load the place's runtime indices (up to TWO) and scale each by its
/// element size: the FIRST loads into r11 (byte-identical to the
/// single-index rung), the SECOND into r10 (the double-index rung). Both
/// load while the base register still equals the region base -- the walk's
/// adds then consume them in step order. A SAME-region index reads through
/// the place's own base register; a CROSS-region index first materializes
/// the index region's base into its own scratch (a recorded relocation
/// site), then loads through it.
fn prepare_place_index(
    bytes: &mut Vec<u8>,
    sites: &mut PlaceCopySites,
    place: &Place,
    base_register: AddressRegister,
    index_sides: (PlaceCopySide, PlaceCopySide),
) -> Result<(), Diagnostic> {
    let mut indices = place.steps().iter().filter_map(|step| match step {
        PlaceStep::ScaledIndex {
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
        } => Some((
            *index_region,
            *index_offset,
            *index_byte_size,
            *element_byte_size,
        )),
        _ => None,
    });
    let Some((index_region, index_offset, index_byte_size, element_byte_size)) = indices.next()
    else {
        return Ok(());
    };
    let second = indices.next();
    if indices.next().is_some() {
        return Err(Diagnostic::error(
            "place materializer: at most two runtime scaled indices per place \
             (r11 and r10 are the index scratches)",
        ));
    }
    if index_region == place.region {
        match base_register {
            AddressRegister::Source => super::append_load_unsigned_reg_from_r14(
                bytes,
                super::Reg64::R11,
                index_offset,
                index_byte_size,
            )?,
            AddressRegister::Target => super::append_load_unsigned_reg_from_r15(
                bytes,
                super::Reg64::R11,
                index_offset,
                index_byte_size,
            )?,
        }
    } else {
        sites.record(bytes.len(), index_sides.0);
        super::append_mov_r11_imm64(bytes, 0);
        super::append_load_unsigned_r11_from_r11(bytes, index_offset, index_byte_size)?;
    }
    super::append_imul_r11_imm32(bytes, super::element_scale(element_byte_size)?);
    if let Some((second_region, second_offset, second_byte_size, second_element)) = second {
        if second_region == place.region {
            match base_register {
                AddressRegister::Source => super::append_load_unsigned_reg_from_r14(
                    bytes,
                    super::Reg64::R10,
                    second_offset,
                    second_byte_size,
                )?,
                AddressRegister::Target => super::append_load_unsigned_reg_from_r15(
                    bytes,
                    super::Reg64::R10,
                    second_offset,
                    second_byte_size,
                )?,
            }
        } else {
            sites.record(bytes.len(), index_sides.1);
            super::append_mov_r10_imm64(bytes, 0);
            super::append_load_unsigned_r10_from_r10(bytes, second_offset, second_byte_size)?;
        }
        super::append_imul_r10_imm32(bytes, super::element_scale(second_element)?);
    }
    Ok(())
}

/// The walk's index consumption: the FIRST ScaledIndex step adds r11, the
/// SECOND adds r10 (loaded/scaled by `prepare_place_index` while the base
/// register still equaled the region base).
fn append_scaled_index_add(bytes: &mut Vec<u8>, register: AddressRegister, ordinal: usize) {
    match (register, ordinal) {
        (AddressRegister::Source, 0) => super::append_add_r14_r11(bytes),
        (AddressRegister::Target, 0) => super::append_add_r15_r11(bytes),
        (AddressRegister::Source, _) => super::append_add_r14_r10(bytes),
        (AddressRegister::Target, _) => super::append_add_r15_r10(bytes),
    }
}

/// Copy `byte_count` bytes from `source` to `target`: materialize both
/// addresses, then move the bytes in aligned 8/4/1 chunks through rax.
pub fn encode_place_copy(
    source: &Place,
    target: &Place,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_place_copy_with_sites(source, target, byte_count).map(|(bytes, _)| bytes)
}

fn encode_place_copy_with_sites(
    source: &Place,
    target: &Place,
    byte_count: usize,
) -> Result<(Vec<u8>, PlaceCopySites), Diagnostic> {
    let mut bytes = Vec::new();
    let mut sites = PlaceCopySites::default();
    let source_displacement =
        materialize_place_address(&mut bytes, &mut sites, source, AddressRegister::Source)?;
    let target_displacement =
        materialize_place_address(&mut bytes, &mut sites, target, AddressRegister::Target)?;
    append_copy_chunks(
        &mut bytes,
        source_displacement,
        target_displacement,
        byte_count,
    )?;
    Ok((bytes, sites))
}

/// A zero-offset place rooted in `region` -- the transitional delegating
/// encoders' construction seed (their kinds carry offsets, not regions, so
/// the region here is documentation; direct-place bytes never consult it).
pub(crate) fn transitional_place(region: omega_target_operations::RuntimeStorageRegion) -> Place {
    Place::at(region, 0)
}

/// The WRITE-family materializer entry (Write rung 1a): store an immediate
/// integer at `byte_size` into a place-shaped target. The target address
/// materializes through the SAME walk as the copy entries (r15 base, the
/// r11/r10 index discipline unchanged); the value stages through rax. For a
/// DIRECT place this is byte-for-byte the retired integer-write layout
/// (`mov r15,imm64(0)`; `mov rax,imm64(value)`; width store).
pub fn encode_place_integer_write(
    target: &Place,
    value: i64,
    byte_size: usize,
) -> Result<(Vec<u8>, PlaceCopySites), Diagnostic> {
    let mut bytes = Vec::new();
    let mut sites = PlaceCopySites::default();
    let displacement =
        materialize_place_address(&mut bytes, &mut sites, target, AddressRegister::Target)?;
    super::append_mov_rax_imm64(&mut bytes, value as u64);
    super::append_store_rax_to_r15(&mut bytes, displacement, byte_size)?;
    Ok((bytes, sites))
}

/// Exact register writes of the generic place integer-write materializer.
/// r15 materializes the destination, r11/r10 carry its first/second runtime
/// indices, and rax carries the immediate value into the sized store.
pub fn place_integer_write_clobbers(target: &Place) -> RegisterSet {
    let indices = target.scaled_index_regions().count();
    let mut registers = vec![MachineRegister::X86Rax, MachineRegister::X86R15];
    if indices > 0 {
        registers.push(MachineRegister::X86R11);
    }
    if indices > 1 {
        registers.push(MachineRegister::X86R10);
    }
    RegisterSet::new(registers)
}

/// The BINARY-write materializer entry (Binary rung 1a): evaluate
/// `left OP right` under the arithmetic domain and store the result into a
/// place-shaped target. The target address materializes through the SAME
/// walk as every place entry (r15 base, r11/r10 indices -- fully consumed
/// into the address BEFORE operands evaluate), then hops to r14 (operand
/// evaluation reloads r15 per source base and clobbers r10/r11); the shared
/// `append_binary_operands_op_and_store` half is the SAME code the retired
/// direct encoder runs.
#[allow(clippy::too_many_arguments)]
pub fn encode_place_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target: &Place,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
    domain: ArithmeticDomain,
    target_signed: bool,
) -> Result<(Vec<u8>, PlaceCopySites), Diagnostic> {
    let mut bytes = Vec::new();
    let mut sites = PlaceCopySites::default();
    let displacement =
        materialize_place_address(&mut bytes, &mut sites, target, AddressRegister::Target)?;
    super::append_mov_r14_r15(&mut bytes);
    super::append_binary_operands_op_and_store(
        runtime_value_operands,
        &mut bytes,
        displacement,
        byte_size,
        left,
        operator,
        right,
        is_float,
        domain,
        target_signed,
    )?;
    Ok((bytes, sites))
}

/// Evaluate one numeric conversion and store it through a composed place.
/// The target address moves to r14 before operand evaluation because recursive
/// operands reload r15 for their own storage bases.
#[allow(clippy::too_many_arguments)]
pub fn encode_place_convert_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target: &Place,
    target_byte_size: usize,
    source: RuntimeValueOperandHandle,
    source_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
    target_signed: bool,
    trapping: bool,
    saturating: bool,
) -> Result<(Vec<u8>, PlaceCopySites), Diagnostic> {
    let mut bytes = Vec::new();
    let mut sites = PlaceCopySites::default();
    let displacement =
        materialize_place_address(&mut bytes, &mut sites, target, AddressRegister::Target)?;
    super::append_mov_r14_r15(&mut bytes);
    super::append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        super::Reg64::R10,
        source,
    )?;
    super::append_runtime_convert_operation(
        &mut bytes,
        source_byte_size,
        target_byte_size,
        source_is_float,
        target_is_float,
        source_signed,
        target_signed,
        trapping,
        saturating,
    );
    super::append_store_r10_to_r14(&mut bytes, displacement, target_byte_size)?;
    Ok((bytes, sites))
}

/// Materialize a scratch text buffer through any x86 place walk. The target
/// descriptor remains in r15 while r14 carries the compiler-owned buffer; the
/// returned buffer site and place sites are emitted by this same byte recipe.
pub fn encode_place_text_buffer_materialize(
    target: &Place,
) -> Result<(Vec<u8>, PlaceCopySites, usize), Diagnostic> {
    let mut bytes = Vec::new();
    let mut sites = PlaceCopySites::default();
    let displacement =
        materialize_place_address(&mut bytes, &mut sites, target, AddressRegister::Target)?;
    let buffer_site = bytes.len();
    super::append_mov_r14_imm64(&mut bytes, 0);
    super::append_load_rax_from_r15(&mut bytes, displacement)?;
    super::append_load_rcx_from_r15(&mut bytes, displacement + 8)?;
    super::append_mov_r11_rcx(&mut bytes);
    super::append_mov_r10_r14(&mut bytes);
    super::append_mov_rsi_rax(&mut bytes);
    super::append_mov_rdi_r10(&mut bytes);
    super::append_rep_movsb(&mut bytes);
    super::append_store_r14_to_r15(&mut bytes, displacement)?;
    super::append_store_r11_to_r15(&mut bytes, displacement + 8)?;
    Ok((bytes, sites, buffer_site))
}

pub fn place_text_buffer_materialize_register_writes() -> RegisterSet {
    super::runtime_text_buffer_materialize_register_writes()
}

pub fn place_text_buffer_materialize_additional_machine_state(target: &Place) -> MachineStateSet {
    if target.scaled_index_regions().next().is_some() {
        MachineStateSet::new([MachineState::Flags])
    } else {
        MachineStateSet::empty()
    }
}

/// Append immediate bytes to the scratch buffer owned by a text descriptor
/// addressed through any x86 place walk.
pub fn encode_place_text_literal_append(
    target: &Place,
    literal: &[u8],
) -> Result<(Vec<u8>, PlaceCopySites, usize), Diagnostic> {
    let mut bytes = Vec::new();
    let mut sites = PlaceCopySites::default();
    let displacement =
        materialize_place_address(&mut bytes, &mut sites, target, AddressRegister::Target)?;
    let buffer_site = bytes.len();
    super::append_mov_r14_imm64(&mut bytes, 0);
    super::append_load_r11_from_r15(&mut bytes, displacement + 8)?;
    for byte in literal {
        bytes.extend([0xb1, *byte]); // mov cl, imm8
        bytes.extend([0x43, 0x88, 0x0c, 0x1e]); // mov [r14+r11], cl
        bytes.extend([0x49, 0xff, 0xc3]); // inc r11
    }
    super::append_store_r14_to_r15(&mut bytes, displacement)?;
    super::append_store_r11_to_r15(&mut bytes, displacement + 8)?;
    Ok((bytes, sites, buffer_site))
}

pub fn place_text_literal_append_register_writes(target: &Place) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::X86Rcx,
        MachineRegister::X86R11,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
    ];
    if target.scaled_index_regions().count() > 1 {
        registers.push(MachineRegister::X86R10);
    }
    RegisterSet::new(registers)
}

/// Append one stored `{ptr,len}` source to the scratch buffer owned by a text
/// descriptor addressed through any x86 place walk.
pub fn encode_place_text_stored_append(
    target: &Place,
    source_offset: usize,
) -> Result<(Vec<u8>, PlaceCopySites, usize, usize), Diagnostic> {
    let mut bytes = Vec::new();
    let mut sites = PlaceCopySites::default();
    let displacement =
        materialize_place_address(&mut bytes, &mut sites, target, AddressRegister::Target)?;
    let buffer_site = bytes.len();
    super::append_mov_r14_imm64(&mut bytes, 0);
    super::append_load_r11_from_r15(&mut bytes, displacement + 8)?;
    super::append_mov_r10_r14(&mut bytes);
    super::append_add_r10_r11(&mut bytes);
    let source_site = bytes.len();
    super::append_mov_rcx_imm64(&mut bytes, 0);
    super::append_load_rax_from_rcx(&mut bytes, source_offset)?;
    super::append_load_rcx_from_rcx(&mut bytes, source_offset + 8)?;
    super::append_add_r11_rcx(&mut bytes);
    super::append_store_r14_to_r15(&mut bytes, displacement)?;
    super::append_store_r11_to_r15(&mut bytes, displacement + 8)?;
    super::append_mov_rsi_rax(&mut bytes);
    super::append_mov_rdi_r10(&mut bytes);
    super::append_rep_movsb(&mut bytes);
    Ok((bytes, sites, buffer_site, source_site))
}

pub fn place_text_stored_append_register_writes() -> RegisterSet {
    super::runtime_text_stored_place_append_register_writes()
}

/// The DETERMINISTIC base-relocation positions of a place binary write's
/// prefix: the target base mov at 0, then each CROSS-REGION index's own
/// base mov at its prep position (index preps run in place order BEFORE the
/// walk; same-region indices load off the target base and add no site).
/// Mirrors `prepare_place_index` + the walk exactly -- the same sums as
/// `place_binary_operand_start_width`.
pub fn place_binary_index_base_positions(
    target: &Place,
) -> impl Iterator<Item = (usize, omega_target_operations::RuntimeStorageRegion)> + '_ {
    let mut width = 10usize; // after the target base mov
    target.steps().iter().filter_map(move |step| match step {
        PlaceStep::ScaledIndex {
            index_region,
            index_byte_size,
            ..
        } => {
            let load_width = super::unsigned_load_width(*index_byte_size);
            let site = if *index_region != target.region {
                let position = width;
                width += 10 + load_width + 7; // mov imm64 + load + imul
                Some((position, *index_region))
            } else {
                width += load_width + 7; // load off the base + imul
                None
            };
            site
        }
        _ => None,
    })
}

/// The byte length of `encode_place_binary_write`'s ADDRESS PREFIX (base
/// mov + index preps + walk adds/derefs + the r14 hop) -- the walker's
/// operand relocations start here. Walk-summed from the materializer's own
/// emission widths, so it can never drift from the bytes.
pub fn place_binary_operand_start_width(target: &Place) -> usize {
    let mut width = 10; // mov r15, imm64
    let mut index_ordinal = 0usize;
    for step in target.steps() {
        match step {
            PlaceStep::ConstOffset(_) => {}
            PlaceStep::Deref => width += 7,
            PlaceStep::ScaledIndex {
                index_region,
                index_byte_size,
                ..
            } => {
                width += if *index_region == target.region {
                    super::unsigned_load_width(*index_byte_size)
                } else {
                    10 + super::unsigned_load_width(*index_byte_size)
                };
                width += 7; // imul
                width += 3; // add at the walk position
                index_ordinal += 1;
            }
        }
    }
    let _ = index_ordinal;
    width + 3 // mov r14, r15
}

/// The TEXT-family materializer entry (Text rung 1a): store a string
/// DESCRIPTOR ({ptr -> rodata, len}) into a place-shaped target. The data
/// pointer stages in r14 via the leading `mov r14,imm64(0)` (the retired
/// convention -- its data-object relocation is ALWAYS at instruction
/// start); the target address materializes through the standard walk
/// (r15 base + the r11/r10 index discipline, base site at +10); the
/// descriptor stores land at [r15 + residual] / +8. A DIRECT place is
/// byte-for-byte the retired machine/frame string layout.
pub fn encode_place_string_write(
    target: &Place,
    byte_length: usize,
) -> Result<(Vec<u8>, PlaceCopySites), Diagnostic> {
    let mut bytes = Vec::new();
    let mut sites = PlaceCopySites::default();
    super::append_mov_r14_imm64(&mut bytes, 0); // data ptr (rodata reloc at +2)
    let displacement =
        materialize_place_address(&mut bytes, &mut sites, target, AddressRegister::Target)?;
    super::append_store_r14_to_r15(&mut bytes, displacement)?;
    super::append_mov_rax_imm64(&mut bytes, byte_length as u64);
    super::append_store_rax_to_r15(&mut bytes, displacement + 8, 8)?;
    Ok((bytes, sites))
}

/// Store one relocated immutable-data address into a direct runtime-frame
/// pointer word. The data relocation owns the leading r14 immediate; the frame
/// relocation owns the ordinary target-place base materialization.
pub fn encode_runtime_frame_data_address_write(
    target_offset: usize,
) -> Result<(Vec<u8>, PlaceCopySites), Diagnostic> {
    encode_runtime_storage_function_address_write(RuntimeStorageRegion::RuntimeFrame, target_offset)
}

/// Store one relocated compiler-private function address into a direct
/// runtime-storage pointer word. Both immediate sites remain symbolic until
/// object relocation.
pub fn encode_runtime_storage_function_address_write(
    target_region: RuntimeStorageRegion,
    target_offset: usize,
) -> Result<(Vec<u8>, PlaceCopySites), Diagnostic> {
    let target = Place::at(target_region, target_offset);
    let mut bytes = Vec::new();
    let mut sites = PlaceCopySites::default();
    super::append_mov_r14_imm64(&mut bytes, 0);
    let displacement =
        materialize_place_address(&mut bytes, &mut sites, &target, AddressRegister::Target)?;
    super::append_store_r14_to_r15(&mut bytes, displacement)?;
    Ok((bytes, sites))
}

/// Register writes of the place-shaped string-descriptor materializer. r14
/// carries the relocated data object, r15 owns the target, rax carries the
/// length, and indexed targets use the normal r11/r10 walk discipline.
pub fn place_string_write_register_writes(target: &Place) -> RegisterSet {
    let mut registers = place_integer_write_clobbers(target).as_slice().to_vec();
    registers.push(MachineRegister::X86R14);
    RegisterSet::new(registers)
}

pub fn place_string_write_additional_machine_state(target: &Place) -> MachineStateSet {
    place_bounded_buffer_write_additional_machine_state(target)
}

/// The BOUNDED-BUFFER materializer entry (Text rung 1e): write a string
/// literal into an owned `[u8; N]` carrier at a place-shaped target -- the
/// len word at [r15 + residual], then the content bytes as IMMEDIATES at
/// [r15 + residual + 8 + i] (`mov byte [r15+disp32], imm8`, 8 bytes each).
/// No data object exists, so the base relocation(s) recorded by the walk
/// are the ONLY sites; a DIRECT place is byte-for-byte the retired machine
/// carrier layout (27 + 8*len) and a pointee place the retired
/// through-pointer layout (34 + 8*len).
pub fn encode_place_bounded_buffer_write(
    target: &Place,
    literal: &[u8],
) -> Result<(Vec<u8>, PlaceCopySites), Diagnostic> {
    let mut bytes = Vec::new();
    let mut sites = PlaceCopySites::default();
    let displacement =
        materialize_place_address(&mut bytes, &mut sites, target, AddressRegister::Target)?;
    super::append_mov_rax_imm64(&mut bytes, literal.len() as u64);
    super::append_store_rax_to_r15(&mut bytes, displacement, 8)?;
    for (index, byte) in literal.iter().enumerate() {
        let content_displacement = super::disp32(displacement + 8 + index)?;
        bytes.extend([0x41, 0xc6, 0x87]); // mov byte [r15 + disp32], imm8
        bytes.extend(content_displacement.to_le_bytes());
        bytes.push(*byte);
    }
    Ok((bytes, sites))
}

/// Register writes of the place-shaped bounded-buffer literal materializer.
/// Its address walk is identical to an immediate integer write, and the
/// length/content stores reuse rax as their only value scratch.
pub fn place_bounded_buffer_write_register_writes(target: &Place) -> RegisterSet {
    place_integer_write_clobbers(target)
}

pub fn place_bounded_buffer_write_additional_machine_state(target: &Place) -> MachineStateSet {
    if target.scaled_index_regions().next().is_some() {
        MachineStateSet::new([MachineState::Flags])
    } else {
        MachineStateSet::empty()
    }
}

/// Append one bounded byte carrier to another after materializing both
/// addresses through the common Place walk. The caller's domain/capacity
/// proof guarantees that the resulting length fits the destination.
pub fn encode_place_bounded_buffer_source_append(
    target: &Place,
    source: &Place,
) -> Result<(Vec<u8>, PlaceCopySites), Diagnostic> {
    let mut bytes = Vec::new();
    let mut sites = PlaceCopySites::default();
    let target_displacement =
        materialize_place_address(&mut bytes, &mut sites, target, AddressRegister::Target)?;
    let source_displacement =
        materialize_place_address(&mut bytes, &mut sites, source, AddressRegister::Source)?;

    super::append_load_rax_from_r15(&mut bytes, target_displacement)?;
    bytes.extend([0x49, 0x8b, 0x8e]); // mov rcx,[r14 + source len]
    bytes.extend(super::disp32(source_displacement)?.to_le_bytes());
    bytes.extend([0x49, 0x8d, 0xbf]); // lea rdi,[r15 + target bytes]
    bytes.extend(super::disp32(target_displacement + 8)?.to_le_bytes());
    bytes.extend([0x48, 0x01, 0xc7]); // add rdi,rax
    bytes.extend([0x48, 0x01, 0xc8]); // add rax,rcx
    bytes.extend([0x49, 0x8d, 0xb6]); // lea rsi,[r14 + source bytes]
    bytes.extend(super::disp32(source_displacement + 8)?.to_le_bytes());
    super::append_rep_movsb(&mut bytes);
    super::append_store_rax_to_r15(&mut bytes, target_displacement, 8)?;
    Ok((bytes, sites))
}

pub fn place_bounded_buffer_source_append_register_writes(
    target: &Place,
    source: &Place,
) -> RegisterSet {
    let mut registers = place_integer_write_clobbers(target).as_slice().to_vec();
    registers.extend_from_slice(place_integer_write_clobbers(source).as_slice());
    registers.extend([
        MachineRegister::X86R14,
        MachineRegister::X86Rcx,
        MachineRegister::X86Rdi,
        MachineRegister::X86Rsi,
    ]);
    RegisterSet::new(registers)
}

pub fn place_bounded_buffer_source_append_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

/// Append immediate literal bytes to a bounded byte carrier addressed through
/// a Place. This is the place-shaped successor of the machine-only encoder.
pub fn encode_place_bounded_buffer_literal_append(
    target: &Place,
    literal: &[u8],
) -> Result<(Vec<u8>, PlaceCopySites), Diagnostic> {
    let mut bytes = Vec::new();
    let mut sites = PlaceCopySites::default();
    let displacement =
        materialize_place_address(&mut bytes, &mut sites, target, AddressRegister::Target)?;
    super::append_load_rax_from_r15(&mut bytes, displacement)?;
    bytes.extend([0x49, 0x8d, 0xbf]); // lea rdi,[r15 + target bytes]
    bytes.extend(super::disp32(displacement + 8)?.to_le_bytes());
    bytes.extend([0x48, 0x01, 0xc7]); // add rdi,rax
    for (index, byte) in literal.iter().enumerate() {
        let index = i8::try_from(index).map_err(|_| {
            Diagnostic::error(
                "X86_64 encoder cannot append a carrier literal longer than 127 bytes",
            )
        })?;
        bytes.extend([0xc6, 0x47, index as u8, *byte]);
    }
    let literal_len = u32::try_from(literal.len()).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 encoder cannot append a carrier literal of {} bytes",
            literal.len()
        ))
    })?;
    bytes.extend([0x48, 0x05]);
    bytes.extend(literal_len.to_le_bytes());
    super::append_store_rax_to_r15(&mut bytes, displacement, 8)?;
    Ok((bytes, sites))
}

pub fn place_bounded_buffer_literal_append_register_writes(target: &Place) -> RegisterSet {
    let mut registers = place_integer_write_clobbers(target).as_slice().to_vec();
    registers.push(MachineRegister::X86Rdi);
    RegisterSet::new(registers)
}

pub fn place_bounded_buffer_literal_append_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

/// The ADDRESS-family materializer entry (task #131): compute the address
/// OF a place-shaped source and store that POINTER into the runtime-frame
/// slot at `target_offset`. The source address rides the standard walk into
/// r15; the residual const offset is then ADDED to r15 (always emitted, so
/// the width is walk-deterministic -- the address IS the payload, nothing
/// folds into a store displacement); the frame slot's own base stages in
/// r14 (`mov r14,imm64`, its relocation at width-17) for the final store.
pub fn encode_place_address_write(
    source: &Place,
    target_offset: usize,
) -> Result<(Vec<u8>, PlaceCopySites), Diagnostic> {
    let mut bytes = Vec::new();
    let mut sites = PlaceCopySites::default();
    let displacement =
        materialize_place_address(&mut bytes, &mut sites, source, AddressRegister::Target)?;
    super::append_add_r15_imm32(&mut bytes, displacement)?; // r15 = full source address
    super::append_mov_r14_imm64(&mut bytes, 0); // target frame base (reloc at width-17)
    super::append_store_r15_to_r14(&mut bytes, target_offset)?; // frame[target] = address
    Ok((bytes, sites))
}

/// Exact register writes of the place-address materializer. The source place
/// walks in r15, its runtime indices use r11/r10 by ordinal, and r14 stages the
/// runtime-frame base for the pointer-slot store.
pub fn place_address_write_register_writes(source: &Place) -> RegisterSet {
    let mut registers = vec![MachineRegister::X86R14, MachineRegister::X86R15];
    let indices = source.scaled_index_regions().count();
    if indices > 0 {
        registers.push(MachineRegister::X86R11);
    }
    if indices > 1 {
        registers.push(MachineRegister::X86R10);
    }
    RegisterSet::new(registers)
}

pub fn place_address_write_additional_machine_state() -> MachineStateSet {
    // The address payload always receives the residual displacement through
    // an ADD, even when that displacement is zero.
    MachineStateSet::new([MachineState::Flags])
}

/// The COMPARE-family materializer entry (task #131, the wiki's
/// guards-consume-Places step): load the LEFT operand through its place
/// (walked in r14, the CopyPlaces source discipline) into r10, the RIGHT
/// through r15 into r11, compare, and emit the guard failure branch.
/// Direct places are position-identical to the retired storage compare
/// (the left leg renames r15 -> r14). REGISTER FENCE: a two-index RIGHT
/// place would clobber r10 (the already-loaded left operand) with its
/// second index scratch -- it refuses loudly (the legalization principle);
/// hoist the subject first.
pub fn encode_place_compare(
    left: &Place,
    right: &Place,
    byte_size: usize,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
    is_float: bool,
) -> Result<(Vec<u8>, PlaceCopySites), Diagnostic> {
    if right.scaled_index_regions().count() >= 2 {
        return Err(Diagnostic::error(
            "a place compare cannot walk a two-index RIGHT operand (its second \
             index scratch would clobber the left operand in r10); hoist the \
             subject to a frame slot first",
        ));
    }
    let mut bytes = Vec::new();
    let mut sites = PlaceCopySites::default();
    let left_displacement =
        materialize_place_address(&mut bytes, &mut sites, left, AddressRegister::Source)?;
    super::append_load_reg_from_r14(&mut bytes, super::Reg64::R10, left_displacement, byte_size)?;
    let right_displacement =
        materialize_place_address(&mut bytes, &mut sites, right, AddressRegister::Target)?;
    super::append_load_reg_from_r15(&mut bytes, super::Reg64::R11, right_displacement, byte_size)?;
    if is_float {
        super::append_float_compare_r10_r11(&mut bytes, byte_size);
    } else {
        super::append_cmp_r10_r11(&mut bytes, byte_size)?;
    }
    super::append_failure_branch(&mut bytes, operator, failure_branch_distance - 4, is_float)?;
    Ok((bytes, sites))
}

/// Exact register writes of the place-pair guard encoder. The materializer
/// owns r14/r15 as address bases and r10/r11 as both index and loaded-value
/// scratch; float comparisons additionally stage their operands in xmm0/1.
pub fn place_compare_register_writes(is_float: bool) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::X86R10,
        MachineRegister::X86R11,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
    ];
    if is_float {
        registers.extend([MachineRegister::X86Xmm(0), MachineRegister::X86Xmm(1)]);
    }
    RegisterSet::new(registers)
}

pub fn place_compare_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

/// The place-vs-immediate compare: the subject loads through its place into
/// r10, the expected value stages in r11 (`mov r11, imm64` -- AFTER the
/// walk, so the walk's r11 index scratch is long consumed), then cmp + the
/// failure branch. A direct place is position-identical to the retired
/// storage-value compare.
pub fn encode_place_value_compare(
    place: &Place,
    byte_size: usize,
    expected_value: i64,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
) -> Result<(Vec<u8>, PlaceCopySites), Diagnostic> {
    let mut bytes = Vec::new();
    let mut sites = PlaceCopySites::default();
    let displacement =
        materialize_place_address(&mut bytes, &mut sites, place, AddressRegister::Target)?;
    super::append_load_reg_from_r15(&mut bytes, super::Reg64::R10, displacement, byte_size)?;
    super::append_mov_reg_imm64(&mut bytes, super::Reg64::R11, expected_value as u64);
    super::append_cmp_r10_r11(&mut bytes, byte_size)?;
    super::append_failure_branch(&mut bytes, operator, failure_branch_distance - 4, false)?;
    Ok((bytes, sites))
}

/// Exact register writes of the place-vs-immediate guard encoder.
pub fn place_value_compare_register_writes() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86R10,
        MachineRegister::X86R11,
        MachineRegister::X86R15,
    ])
}

pub fn place_value_compare_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

/// The `CopyPlaces` entry: ONE routine that picks the emission shape from the
/// place pair itself -- shared-base only when both places root in the SAME
/// region and the pair satisfies that walk's structural constraints (the
/// shape every retired same-region indexed/pointee encoder hand-spelled),
/// two-base otherwise. Same-region direct/indexed pairs still materialize two
/// independently patched bases, just as the retired plain copy did. Returns
/// the bytes AND the base relocation sites recorded by the same walk; the
/// relocation walker patches each site from the corresponding place's own
/// region.
pub fn encode_copy_places(
    source: &Place,
    target: &Place,
    byte_count: usize,
) -> Result<(Vec<u8>, PlaceCopySites), Diagnostic> {
    if place_pair_can_share_base(source, target) {
        encode_place_copy_shared_base_with_sites(source, target, byte_count)
    } else {
        encode_place_copy_with_sites(source, target, byte_count)
    }
}

fn place_pair_can_share_base(source: &Place, target: &Place) -> bool {
    if source.region != target.region {
        return false;
    }
    let source_derefs = place_derefs(source);
    let target_derefs = place_derefs(target);
    if !source_derefs && !target_derefs {
        return false;
    }
    let source_indexed = place_has_index(source);
    let target_indexed = place_has_index(target);
    if (source_indexed && target_indexed)
        || (source_indexed && !source_derefs)
        || (target_indexed && !target_derefs)
    {
        return false;
    }

    // The source is the hopping side whenever it dereferences; otherwise the
    // target hops. An index before that first deref would mutate the shared
    // register before the hop has preserved the original base.
    let hopping_side = if source_derefs { source } else { target };
    hopping_side
        .steps()
        .iter()
        .position(|step| matches!(step, PlaceStep::Deref))
        .is_some_and(|first_deref| {
            !hopping_side.steps()[..first_deref]
                .iter()
                .any(|step| matches!(step, PlaceStep::ScaledIndex { .. }))
        })
}

/// Exact scratch footprint of the generic `CopyPlaces` materializer for any
/// successfully encoded place pair. Both address registers are always
/// written; r11 carries the first runtime index on either side, r10 carries a
/// second index within either place, and non-empty chunks stage through rax.
pub fn copy_places_clobbers(source: &Place, target: &Place, byte_count: usize) -> RegisterSet {
    let source_indices = source.scaled_index_regions().count();
    let target_indices = target.scaled_index_regions().count();
    let mut registers = vec![MachineRegister::X86R14, MachineRegister::X86R15];
    if source_indices > 0 || target_indices > 0 {
        registers.push(MachineRegister::X86R11);
    }
    if source_indices > 1 || target_indices > 1 {
        registers.push(MachineRegister::X86R10);
    }
    if byte_count > 0 {
        registers.push(MachineRegister::X86Rax);
    }
    RegisterSet::new(registers)
}

/// Exact scratch footprint of a direct-source to dereferenced-target copy,
/// the shape used by an indirect boundary result. The shared-base materializer
/// holds the source base in r14, hops the target pointer into r15, and stages
/// non-empty chunks through rax.
pub fn copy_places_to_pointee_clobbers(byte_count: usize) -> RegisterSet {
    let mut registers = vec![MachineRegister::X86R14, MachineRegister::X86R15];
    if byte_count > 0 {
        registers.push(MachineRegister::X86Rax);
    }
    RegisterSet::new(registers)
}

/// Exact scratch footprint of a dereferenced-source to direct-target copy.
/// The shared-base materializer starts in r15, hops the source pointer into
/// r14, and stages non-empty chunks through rax.
pub fn copy_places_from_pointee_clobbers(byte_count: usize) -> RegisterSet {
    copy_places_direct_clobbers(byte_count)
}

/// Exact scratch footprint of a dereferenced-source to dereferenced-target
/// copy. The shared-base walk holds the two pointees in r14/r15 and stages
/// non-empty chunks through rax.
pub fn copy_places_pointee_pair_clobbers(byte_count: usize) -> RegisterSet {
    copy_places_direct_clobbers(byte_count)
}

/// Exact scratch footprint of a copy with one runtime-indexed source. r11
/// holds and scales the index, r14/r15 hold the two addresses, and non-empty
/// chunks stage through rax.
pub fn copy_places_from_indexed_clobbers(byte_count: usize) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::X86R11,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
    ];
    if byte_count > 0 {
        registers.push(MachineRegister::X86Rax);
    }
    RegisterSet::new(registers)
}

/// Exact scratch footprint of a copy with one runtime-indexed target. It uses
/// the same one-index place-walk contract as the source-indexed mirror.
pub fn copy_places_to_indexed_clobbers(byte_count: usize) -> RegisterSet {
    copy_places_from_indexed_clobbers(byte_count)
}

/// Exact scratch footprint of a runtime-indexed source copied through a
/// pointee target. The shared-base walk adds the same one-index scratch to the
/// two address registers used by the other indexed copy shapes.
pub fn copy_places_indexed_to_pointee_clobbers(byte_count: usize) -> RegisterSet {
    copy_places_from_indexed_clobbers(byte_count)
}

/// Exact scratch footprint of a frame-resident inline-array element read into
/// direct frame storage. The two-base materializer uses the standard
/// one-index walk even though every relocation site names the frame region.
pub fn copy_places_from_frame_base_indexed_clobbers(byte_count: usize) -> RegisterSet {
    copy_places_from_indexed_clobbers(byte_count)
}

/// Exact scratch footprint of a machine-inline-array element read into direct
/// storage. The generic materializer again performs one indexed source walk
/// followed by the direct target address.
pub fn copy_places_from_machine_indexed_clobbers(byte_count: usize) -> RegisterSet {
    copy_places_from_indexed_clobbers(byte_count)
}

/// Exact scratch footprint of a direct-storage value written into a
/// machine-inline-array element.
pub fn copy_places_to_machine_indexed_clobbers(byte_count: usize) -> RegisterSet {
    copy_places_from_indexed_clobbers(byte_count)
}

/// Exact scratch footprint of an inline frame-array double-indexed read. Both
/// indices use the materializer's distinct r11/r10 scratches.
pub fn copy_places_from_frame_base_double_indexed_clobbers(byte_count: usize) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::X86R10,
        MachineRegister::X86R11,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
    ];
    if byte_count > 0 {
        registers.push(MachineRegister::X86Rax);
    }
    RegisterSet::new(registers)
}

/// Exact scratch footprint of a machine-rooted double-indexed read.
pub fn copy_places_from_machine_double_indexed_clobbers(byte_count: usize) -> RegisterSet {
    copy_places_from_frame_base_double_indexed_clobbers(byte_count)
}

/// Exact scratch footprint of a direct-storage value written through two
/// runtime machine-array indices.
pub fn copy_places_to_machine_double_indexed_clobbers(byte_count: usize) -> RegisterSet {
    copy_places_from_frame_base_double_indexed_clobbers(byte_count)
}

/// Exact scratch footprint of `machine[i] = machine[j]`. Each side has one
/// runtime index, and the generic two-base materializer reuses r11 for the
/// two walks; r10 is reserved for a second index within ONE place and is not
/// written by this shape.
pub fn copy_places_machine_indexed_pair_clobbers(byte_count: usize) -> RegisterSet {
    copy_places_from_indexed_clobbers(byte_count)
}

/// Exact scratch footprint of a direct place-pair copy. Both address bases are
/// materialized unconditionally; non-empty copies stage chunks through rax.
pub fn copy_places_direct_clobbers(byte_count: usize) -> RegisterSet {
    let mut registers = vec![MachineRegister::X86R14, MachineRegister::X86R15];
    if byte_count > 0 {
        registers.push(MachineRegister::X86Rax);
    }
    RegisterSet::new(registers)
}

/// The SHARED-BASE copy: both places root in the SAME region, so ONE base
/// materialization (into r15) serves both -- each place's FIRST deref loads
/// its own address register THROUGH the shared base, source before target
/// (the source's pointer must be read before the target's deref consumes
/// r15). Requires the source to start with a deref (after any const prefix);
/// the target may be a pure-const path (r15 stays the base) or start with a
/// deref of its own. This is the one-relocation shape the retired same-region
/// indexed/pointee encoders hand-spelled; the region stays documentation on
/// the transitional path (callers pick this entry where the retired encoder
/// shared its base, so walker byte math is unchanged).
pub fn encode_place_copy_shared_base(
    source: &Place,
    target: &Place,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_place_copy_shared_base_with_sites(source, target, byte_count).map(|(bytes, _)| bytes)
}

fn encode_place_copy_shared_base_with_sites(
    source: &Place,
    target: &Place,
    byte_count: usize,
) -> Result<(Vec<u8>, PlaceCopySites), Diagnostic> {
    let source_derefs = place_derefs(source);
    let target_derefs = place_derefs(target);
    let source_indexed = place_has_index(source);
    let target_indexed = place_has_index(target);
    if source_indexed && target_indexed {
        return Err(Diagnostic::error(
            "shared-base place copy: only one side may carry a runtime index \
             (r11 is the single index scratch)",
        ));
    }
    if (source_indexed && !source_derefs) || (target_indexed && !target_derefs) {
        return Err(Diagnostic::error(
            "shared-base place copy: a runtime index is only legal on a \
             dereferencing side (an index add on a direct side would mutate the \
             shared base) -- route this pair through encode_place_copy",
        ));
    }

    let mut bytes = Vec::new();
    let mut sites = PlaceCopySites::default();
    if source_derefs {
        // Base lives in r15 (the target register); the first source deref
        // hops its address to r14 BEFORE any target deref consumes r15.
        // The single base serves BOTH places (same region by precondition);
        // the site carries the register's own side.
        sites.record(bytes.len(), PlaceCopySide::Target);
        super::append_mov_r15_imm64(&mut bytes, 0);
        prepare_place_index(
            &mut bytes,
            &mut sites,
            if source_indexed { source } else { target },
            AddressRegister::Target,
            if source_indexed {
                (PlaceCopySide::SourceIndex, PlaceCopySide::SourceIndex2)
            } else {
                (PlaceCopySide::TargetIndex, PlaceCopySide::TargetIndex2)
            },
        )?;
        let source_displacement =
            walk_hopping_side(&mut bytes, source, HopDirection::BaseR15SourceHops)?;
        let target_displacement = walk_base_side(&mut bytes, target, AddressRegister::Target)?;
        append_copy_chunks(
            &mut bytes,
            source_displacement,
            target_displacement,
            byte_count,
        )?;
        Ok((bytes, sites))
    } else if target_derefs {
        // The mirror: the source is direct, so the base lives in r14 (the
        // source register) and the first target deref hops to r15.
        sites.record(bytes.len(), PlaceCopySide::Source);
        super::append_mov_r14_imm64(&mut bytes, 0);
        prepare_place_index(
            &mut bytes,
            &mut sites,
            target,
            AddressRegister::Source,
            (PlaceCopySide::TargetIndex, PlaceCopySide::TargetIndex2),
        )?;
        let target_displacement =
            walk_hopping_side(&mut bytes, target, HopDirection::BaseR14TargetHops)?;
        let source_displacement = walk_base_side(&mut bytes, source, AddressRegister::Source)?;
        append_copy_chunks(
            &mut bytes,
            source_displacement,
            target_displacement,
            byte_count,
        )?;
        Ok((bytes, sites))
    } else {
        Err(Diagnostic::error(
            "shared-base place copy requires a dereferencing side -- \
             a direct pair routes through encode_place_copy",
        ))
    }
}

fn place_derefs(place: &Place) -> bool {
    place
        .steps()
        .iter()
        .any(|step| matches!(step, PlaceStep::Deref))
}

fn place_has_index(place: &Place) -> bool {
    place
        .steps()
        .iter()
        .any(|step| matches!(step, PlaceStep::ScaledIndex { .. }))
}

#[derive(Clone, Copy)]
enum HopDirection {
    /// The shared base is r15; the hopping side address lands in r14.
    BaseR15SourceHops,
    /// The shared base is r14; the hopping side address lands in r15.
    BaseR14TargetHops,
}

/// Walk the side whose first deref HOPS off the shared base into its own
/// register; subsequent steps continue there. Returns the residual
/// displacement. An index add fires only after the hop (enforced by the
/// dereferencing-side check above).
fn walk_hopping_side(
    bytes: &mut Vec<u8>,
    place: &Place,
    direction: HopDirection,
) -> Result<usize, Diagnostic> {
    let mut steps = place.steps().iter();
    let mut prefix = 0usize;
    loop {
        match steps.next() {
            Some(PlaceStep::ConstOffset(offset)) => prefix += offset,
            Some(PlaceStep::Deref) => break,
            Some(PlaceStep::ScaledIndex { .. }) => {
                return Err(Diagnostic::error(
                    "shared-base place copy: a runtime index cannot precede the \
                     hopping deref (the add would target the shared base)",
                ));
            }
            None => unreachable!("walk_hopping_side requires a dereferencing place"),
        }
    }
    match direction {
        HopDirection::BaseR15SourceHops => super::append_load_r14_from_r15(bytes, prefix)?,
        HopDirection::BaseR14TargetHops => super::append_load_r15_from_r14(bytes, prefix)?,
    }
    let own_register = match direction {
        HopDirection::BaseR15SourceHops => AddressRegister::Source,
        HopDirection::BaseR14TargetHops => AddressRegister::Target,
    };
    let mut displacement = 0usize;
    let mut index_ordinal = 0usize;
    for step in steps {
        match step {
            PlaceStep::ConstOffset(offset) => displacement += offset,
            PlaceStep::Deref => {
                match own_register {
                    AddressRegister::Source => {
                        super::append_load_r14_from_r14(bytes, displacement)?
                    }
                    AddressRegister::Target => {
                        super::append_load_r15_from_r15(bytes, displacement)?
                    }
                }
                displacement = 0;
            }
            PlaceStep::ScaledIndex { .. } => {
                append_scaled_index_add(bytes, own_register, index_ordinal);
                index_ordinal += 1;
            }
        }
    }
    Ok(displacement)
}

/// Walk the side that stays ON the shared base register (its derefs, if any,
/// consume the base in place -- legal because the hopping side already left).
fn walk_base_side(
    bytes: &mut Vec<u8>,
    place: &Place,
    register: AddressRegister,
) -> Result<usize, Diagnostic> {
    let mut displacement = 0usize;
    let mut index_ordinal = 0usize;
    for step in place.steps() {
        match step {
            PlaceStep::ConstOffset(offset) => displacement += offset,
            PlaceStep::Deref => {
                match register {
                    AddressRegister::Source => {
                        super::append_load_r14_from_r14(bytes, displacement)?
                    }
                    AddressRegister::Target => {
                        super::append_load_r15_from_r15(bytes, displacement)?
                    }
                }
                displacement = 0;
            }
            PlaceStep::ScaledIndex { .. } => {
                append_scaled_index_add(bytes, register, index_ordinal);
                index_ordinal += 1;
            }
        }
    }
    Ok(displacement)
}

fn append_copy_chunks(
    bytes: &mut Vec<u8>,
    source_displacement: usize,
    target_displacement: usize,
    byte_count: usize,
) -> Result<(), Diagnostic> {
    super::for_each_runtime_copy_chunk(
        source_displacement,
        target_displacement,
        byte_count,
        |offset, chunk_size| {
            super::append_load_rax_from_r14(bytes, source_displacement + offset, chunk_size)?;
            super::append_store_rax_to_r15(bytes, target_displacement + offset, chunk_size)?;
            Ok(())
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_target_operations::RuntimeStorageRegion;

    /// The delegated plain copy must be byte-for-byte the retired encoder's
    /// output: base movs then aligned chunks with folded displacements.
    #[test]
    fn plain_copy_matches_the_retired_layout() {
        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 16);
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 40);
        let bytes = encode_place_copy(&source, &target, 16).expect("const path encodes");

        let mut expected = Vec::new();
        super::super::append_mov_r14_imm64(&mut expected, 0);
        super::super::append_mov_r15_imm64(&mut expected, 0);
        super::super::for_each_runtime_copy_chunk(16, 40, 16, |offset, chunk_size| {
            super::super::append_load_rax_from_r14(&mut expected, 16 + offset, chunk_size)?;
            super::super::append_store_rax_to_r15(&mut expected, 40 + offset, chunk_size)?;
            Ok(())
        })
        .expect("chunks");
        assert_eq!(bytes, expected);
    }

    #[test]
    fn direct_copy_clobbers_track_empty_and_nonempty_chunks() {
        assert_eq!(
            copy_places_direct_clobbers(0).as_slice(),
            &[MachineRegister::X86R14, MachineRegister::X86R15]
        );
        assert_eq!(
            copy_places_direct_clobbers(8).as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
    }

    #[test]
    fn place_integer_write_clobbers_follow_target_index_depth() {
        let direct = Place::at(RuntimeStorageRegion::RuntimeFrame, 16);
        assert_eq!(
            place_integer_write_clobbers(&direct).as_slice(),
            &[MachineRegister::X86Rax, MachineRegister::X86R15]
        );

        let indexed = direct
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 64,
                index_byte_size: 8,
                element_byte_size: 24,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::Machine,
                    index_offset: 80,
                    index_byte_size: 4,
                    element_byte_size: 8,
                })
            })
            .expect("double-indexed target");
        assert_eq!(
            place_integer_write_clobbers(&indexed).as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86R10,
                MachineRegister::X86R11,
                MachineRegister::X86R15,
            ]
        );
    }

    #[test]
    fn generic_copy_clobbers_follow_each_places_index_depth() {
        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 16)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 64,
                index_byte_size: 8,
                element_byte_size: 24,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::Machine,
                    index_offset: 80,
                    index_byte_size: 4,
                    element_byte_size: 8,
                })
            })
            .expect("double-indexed source");
        let target = Place::at(RuntimeStorageRegion::Machine, 32)
            .with_step(PlaceStep::Deref)
            .expect("pointee target");
        assert_eq!(
            copy_places_clobbers(&source, &target, 8).as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86R10,
                MachineRegister::X86R11,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
    }

    #[test]
    fn from_pointee_clobbers_track_empty_and_nonempty_chunks() {
        assert_eq!(
            copy_places_from_pointee_clobbers(0).as_slice(),
            &[MachineRegister::X86R14, MachineRegister::X86R15]
        );
        assert_eq!(
            copy_places_from_pointee_clobbers(8).as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
    }

    #[test]
    fn pointee_pair_clobbers_track_empty_and_nonempty_chunks() {
        assert_eq!(
            copy_places_pointee_pair_clobbers(0).as_slice(),
            &[MachineRegister::X86R14, MachineRegister::X86R15]
        );
        assert_eq!(
            copy_places_pointee_pair_clobbers(8).as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
    }

    #[test]
    fn from_indexed_clobbers_track_index_and_nonempty_chunks() {
        assert_eq!(
            copy_places_from_indexed_clobbers(0).as_slice(),
            &[
                MachineRegister::X86R11,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
        assert_eq!(
            copy_places_from_indexed_clobbers(8).as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86R11,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
    }

    #[test]
    fn to_indexed_clobbers_match_the_one_index_place_walk() {
        assert_eq!(
            copy_places_to_indexed_clobbers(0),
            copy_places_from_indexed_clobbers(0)
        );
        assert_eq!(
            copy_places_to_indexed_clobbers(8),
            copy_places_from_indexed_clobbers(8)
        );
    }

    #[test]
    fn indexed_to_pointee_clobbers_match_the_one_index_place_walk() {
        assert_eq!(
            copy_places_indexed_to_pointee_clobbers(0),
            copy_places_from_indexed_clobbers(0)
        );
        assert_eq!(
            copy_places_indexed_to_pointee_clobbers(8),
            copy_places_from_indexed_clobbers(8)
        );
    }

    #[test]
    fn frame_base_indexed_clobbers_match_the_one_index_place_walk() {
        assert_eq!(
            copy_places_from_frame_base_indexed_clobbers(0),
            copy_places_from_indexed_clobbers(0)
        );
        assert_eq!(
            copy_places_from_frame_base_indexed_clobbers(8),
            copy_places_from_indexed_clobbers(8)
        );
    }

    #[test]
    fn machine_indexed_clobbers_match_the_one_index_place_walk() {
        assert_eq!(
            copy_places_from_machine_indexed_clobbers(0),
            copy_places_from_indexed_clobbers(0)
        );
        assert_eq!(
            copy_places_from_machine_indexed_clobbers(8),
            copy_places_from_indexed_clobbers(8)
        );
    }

    #[test]
    fn to_machine_indexed_clobbers_match_the_one_index_place_walk() {
        assert_eq!(
            copy_places_to_machine_indexed_clobbers(0),
            copy_places_from_indexed_clobbers(0)
        );
        assert_eq!(
            copy_places_to_machine_indexed_clobbers(8),
            copy_places_from_indexed_clobbers(8)
        );
    }

    #[test]
    fn frame_double_indexed_clobbers_include_both_index_scratches() {
        assert_eq!(
            copy_places_from_frame_base_double_indexed_clobbers(8).as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86R10,
                MachineRegister::X86R11,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
    }

    /// A deref step emits the pointer load exactly where the retired pointee
    /// encoders placed it, and the post-deref offset folds into the chunks.
    #[test]
    fn target_deref_matches_the_pointee_layout() {
        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 8);
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
            .with_step(PlaceStep::Deref)
            .unwrap()
            .with_step(PlaceStep::ConstOffset(4))
            .unwrap();
        let bytes = encode_place_copy(&source, &target, 4).expect("deref path encodes");

        let mut expected = Vec::new();
        super::super::append_mov_r14_imm64(&mut expected, 0);
        super::super::append_mov_r15_imm64(&mut expected, 0);
        super::super::append_load_r15_from_r15(&mut expected, 32).expect("pointer load");
        super::super::for_each_runtime_copy_chunk(8, 4, 4, |offset, chunk_size| {
            super::super::append_load_rax_from_r14(&mut expected, 8 + offset, chunk_size)?;
            super::super::append_store_rax_to_r15(&mut expected, 4 + offset, chunk_size)?;
            Ok(())
        })
        .expect("chunks");
        assert_eq!(bytes, expected);
    }

    /// The shared-base shape with a pure-const target must be byte-for-byte
    /// the retired fixed-indexed-to-frame encoder: one base mov, the
    /// descriptor deref hops the source to r14, chunks fold the offsets.
    #[test]
    fn shared_base_const_target_matches_the_fixed_indexed_layout() {
        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 48)
            .with_step(PlaceStep::Deref)
            .unwrap()
            .with_step(PlaceStep::ConstOffset(12))
            .unwrap();
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 96);
        let bytes = encode_place_copy_shared_base(&source, &target, 8).expect("encodes");

        let mut expected = Vec::new();
        super::super::append_mov_r15_imm64(&mut expected, 0);
        super::super::append_load_r14_from_r15(&mut expected, 48).expect("deref");
        super::super::for_each_runtime_copy_chunk(12, 96, 8, |offset, chunk_size| {
            super::super::append_load_rax_from_r14(&mut expected, 12 + offset, chunk_size)?;
            super::super::append_store_rax_to_r15(&mut expected, 96 + offset, chunk_size)?;
            Ok(())
        })
        .expect("chunks");
        assert_eq!(bytes, expected);
    }

    /// The shared-base shape with BOTH sides dereferencing must order the
    /// source's pointer read BEFORE the target's deref consumes r15 -- the
    /// retired fixed-indexed-to-pointee layout.
    #[test]
    fn shared_base_double_deref_matches_the_pointee_layout() {
        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 48)
            .with_step(PlaceStep::Deref)
            .unwrap()
            .with_step(PlaceStep::ConstOffset(12))
            .unwrap();
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 72)
            .with_step(PlaceStep::Deref)
            .unwrap()
            .with_step(PlaceStep::ConstOffset(4))
            .unwrap();
        let bytes = encode_place_copy_shared_base(&source, &target, 4).expect("encodes");

        let mut expected = Vec::new();
        super::super::append_mov_r15_imm64(&mut expected, 0);
        super::super::append_load_r14_from_r15(&mut expected, 48).expect("source deref");
        super::super::append_load_r15_from_r15(&mut expected, 72).expect("target deref");
        super::super::for_each_runtime_copy_chunk(12, 4, 4, |offset, chunk_size| {
            super::super::append_load_rax_from_r14(&mut expected, 12 + offset, chunk_size)?;
            super::super::append_store_rax_to_r15(&mut expected, 4 + offset, chunk_size)?;
            Ok(())
        })
        .expect("chunks");
        assert_eq!(bytes, expected);
    }

    /// A direct (deref-free) source is not a shared-base shape -- the entry
    /// refuses instead of silently emitting a wrong-base copy.
    #[test]
    fn shared_base_refuses_a_direct_source() {
        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 8);
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 16);
        assert!(encode_place_copy_shared_base(&source, &target, 8).is_err());
    }

    /// The canonical two-base shape pins the target relocation position the
    /// walker's fixed-indexed-to-storage arm mirrors: source base (10) +
    /// descriptor deref (7) puts the target base mov at +17 (imm at +19).
    #[test]
    fn canonical_deref_source_puts_the_target_base_at_17() {
        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 40)
            .with_step(PlaceStep::Deref)
            .unwrap()
            .with_step(PlaceStep::ConstOffset(8))
            .unwrap();
        let target = Place::at(RuntimeStorageRegion::Machine, 64);
        let bytes = encode_place_copy(&source, &target, 8).expect("encodes");
        assert_eq!(super::super::FRAME_FIXED_INDEXED_COPY_TARGET_IMM_OFFSET, 17);
        // 49 BF = mov r15, imm64 at the pinned offset.
        assert_eq!(&bytes[17..19], &[0x49, 0xbf]);
    }

    /// The shared-base runtime-indexed source (the from_frame_indexed
    /// family): the index loads from the SHARED base and scales BEFORE the
    /// hopping deref, then adds onto the hopped source address.
    #[test]
    fn shared_base_indexed_source_layout() {
        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 48)
            .with_step(PlaceStep::Deref)
            .unwrap()
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 56,
                index_byte_size: 4,
                element_byte_size: 4,
            })
            .unwrap()
            .with_step(PlaceStep::ConstOffset(8))
            .unwrap();
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 96);
        let bytes = encode_place_copy_shared_base(&source, &target, 4).expect("encodes");

        let mut expected = Vec::new();
        super::super::append_mov_r15_imm64(&mut expected, 0);
        super::super::append_load_unsigned_reg_from_r15(
            &mut expected,
            super::super::Reg64::R11,
            56,
            4,
        )
        .expect("index");
        super::super::append_imul_r11_imm32(&mut expected, 4);
        super::super::append_load_r14_from_r15(&mut expected, 48).expect("hop");
        super::super::append_add_r14_r11(&mut expected);
        super::super::for_each_runtime_copy_chunk(8, 96, 4, |offset, chunk_size| {
            super::super::append_load_rax_from_r14(&mut expected, 8 + offset, chunk_size)?;
            super::super::append_store_rax_to_r15(&mut expected, 96 + offset, chunk_size)?;
            Ok(())
        })
        .expect("chunks");
        assert_eq!(bytes, expected);
    }

    /// The mirror: a runtime-indexed TARGET (the to_frame_indexed write face
    /// the old product never built on x86_64) -- base in r14, the index loads
    /// from it, the target hops to r15 and adds the scaled index.
    #[test]
    fn shared_base_indexed_target_layout() {
        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 24);
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 48)
            .with_step(PlaceStep::Deref)
            .unwrap()
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 56,
                index_byte_size: 4,
                element_byte_size: 8,
            })
            .unwrap();
        let bytes = encode_place_copy_shared_base(&source, &target, 8).expect("encodes");

        let mut expected = Vec::new();
        super::super::append_mov_r14_imm64(&mut expected, 0);
        super::super::append_load_unsigned_reg_from_r14(
            &mut expected,
            super::super::Reg64::R11,
            56,
            4,
        )
        .expect("index");
        super::super::append_imul_r11_imm32(&mut expected, 8);
        super::super::append_load_r15_from_r14(&mut expected, 48).expect("hop");
        super::super::append_add_r15_r11(&mut expected);
        super::super::for_each_runtime_copy_chunk(24, 0, 8, |offset, chunk_size| {
            super::super::append_load_rax_from_r14(&mut expected, 24 + offset, chunk_size)?;
            super::super::append_store_rax_to_r15(&mut expected, offset, chunk_size)?;
            Ok(())
        })
        .expect("chunks");
        assert_eq!(bytes, expected);
    }

    /// The CROSS-REGION index (rung 2c-vii, the machine-indexed family): a
    /// MACHINE-region array indexed by a FRAME-resident slot. r11 first
    /// materializes the index region's base (a recorded SourceIndex
    /// relocation site), then loads the index through itself -- no extra
    /// scratch register. The machine base has no deref (inline array), so
    /// the scaled add fires at the step's walk position.
    #[test]
    fn cross_region_index_materializes_its_own_base() {
        let source = Place::at(RuntimeStorageRegion::Machine, 32)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 16,
                index_byte_size: 4,
                element_byte_size: 4,
            })
            .unwrap()
            .with_step(PlaceStep::ConstOffset(0))
            .unwrap();
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 64);
        let (bytes, sites) = encode_copy_places(&source, &target, 4).expect("encodes");

        let mut expected = Vec::new();
        super::super::append_mov_r14_imm64(&mut expected, 0); // machine base (Source site @0)
        let index_base_offset = expected.len();
        super::super::append_mov_r11_imm64(&mut expected, 0); // frame base for the index
        super::super::append_load_unsigned_r11_from_r11(&mut expected, 16, 4).expect("index");
        super::super::append_imul_r11_imm32(&mut expected, 4);
        super::super::append_add_r14_r11(&mut expected);
        let target_base_offset = expected.len();
        super::super::append_mov_r15_imm64(&mut expected, 0); // frame target base
        super::super::for_each_runtime_copy_chunk(32, 64, 4, |offset, chunk_size| {
            super::super::append_load_rax_from_r14(&mut expected, 32 + offset, chunk_size)?;
            super::super::append_store_rax_to_r15(&mut expected, 64 + offset, chunk_size)?;
            Ok(())
        })
        .expect("chunks");
        assert_eq!(bytes, expected);

        let recorded: Vec<(usize, PlaceCopySide)> = sites.iter().collect();
        assert_eq!(
            recorded,
            vec![
                (0, PlaceCopySide::Source),
                (index_base_offset, PlaceCopySide::SourceIndex),
                (target_base_offset, PlaceCopySide::Target),
            ]
        );
    }

    /// Index refusals: two indices on one place; both sides indexed; an index
    /// on a direct shared side.
    #[test]
    fn scaled_index_refusals() {
        let indexed = |offset: usize| {
            Place::at(RuntimeStorageRegion::RuntimeFrame, offset)
                .with_step(PlaceStep::Deref)
                .unwrap()
                .with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::RuntimeFrame,
                    index_offset: 8,
                    index_byte_size: 4,
                    element_byte_size: 4,
                })
                .unwrap()
        };
        // Two indices on one place: LEGAL since the double-index rung (r10
        // is the second scratch); three refuse (triple_index_refuses).
        let double = indexed(0)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 16,
                index_byte_size: 4,
                element_byte_size: 4,
            })
            .unwrap();
        let plain = Place::at(RuntimeStorageRegion::Machine, 64);
        assert!(encode_place_copy(&double, &plain, 4).is_ok());
        // Both sides indexed (shared base).
        assert!(encode_place_copy_shared_base(&indexed(0), &indexed(32), 4).is_err());
        // Index on a DIRECT side (shared base): would mutate the shared base.
        let direct_indexed = Place::at(RuntimeStorageRegion::RuntimeFrame, 0)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 8,
                index_byte_size: 4,
                element_byte_size: 4,
            })
            .unwrap();
        assert!(encode_place_copy_shared_base(&direct_indexed, &indexed(32), 4).is_err());
    }

    /// `CopyPlaces` must not confuse a common region with an encodable
    /// one-base shape. Two runtime-indexed sides need independent bases and
    /// retain one relocation site per side even when both patch to the frame.
    #[test]
    fn same_region_indexed_pair_routes_through_two_bases() {
        let indexed = |offset: usize, index_offset: usize| {
            Place::at(RuntimeStorageRegion::RuntimeFrame, offset)
                .with_step(PlaceStep::Deref)
                .unwrap()
                .with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::RuntimeFrame,
                    index_offset,
                    index_byte_size: 4,
                    element_byte_size: 4,
                })
                .unwrap()
        };
        let (bytes, sites) =
            encode_copy_places(&indexed(0, 8), &indexed(32, 40), 4).expect("encodes");

        assert_eq!(&bytes[..2], &[0x49, 0xbe], "source base starts in r14");
        let sides = sites.iter().map(|(_, side)| side).collect::<Vec<_>>();
        assert_eq!(sides, vec![PlaceCopySide::Source, PlaceCopySide::Target]);
    }

    /// A runtime index before the hopping deref cannot use the shared base,
    /// but the generic materializer can preserve the pair with two bases.
    #[test]
    fn same_region_pre_deref_index_routes_through_two_bases() {
        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 0)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 8,
                index_byte_size: 4,
                element_byte_size: 16,
            })
            .unwrap()
            .with_step(PlaceStep::Deref)
            .unwrap();
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 32);

        assert!(encode_place_copy_shared_base(&source, &target, 4).is_err());
        let (_, sites) = encode_copy_places(&source, &target, 4).expect("encodes");
        let sides = sites.iter().map(|(_, side)| side).collect::<Vec<_>>();
        assert_eq!(sides, vec![PlaceCopySide::Source, PlaceCopySide::Target]);
    }

    /// The double-index rung: a machine-style no-deref place with TWO
    /// same-region ScaledIndex steps -- both indices pre-load (r11 first,
    /// r10 second) while the base register still equals the region base;
    /// the walk consumes them in step order.
    #[test]
    fn double_index_same_region_layout() {
        let source = Place::at(RuntimeStorageRegion::Machine, 32)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 8,
                index_byte_size: 4,
                element_byte_size: 16,
            })
            .unwrap()
            .with_step(PlaceStep::ConstOffset(4))
            .unwrap()
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 12,
                index_byte_size: 4,
                element_byte_size: 4,
            })
            .unwrap();
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 64);
        let (bytes, sites) = encode_copy_places(&source, &target, 4).expect("encodes");

        let mut expected = Vec::new();
        super::super::append_mov_r14_imm64(&mut expected, 0);
        super::super::append_load_unsigned_reg_from_r14(
            &mut expected,
            super::super::Reg64::R11,
            8,
            4,
        )
        .expect("first index");
        super::super::append_imul_r11_imm32(&mut expected, 16);
        super::super::append_load_unsigned_reg_from_r14(
            &mut expected,
            super::super::Reg64::R10,
            12,
            4,
        )
        .expect("second index");
        super::super::append_imul_r10_imm32(&mut expected, 4);
        super::super::append_add_r14_r11(&mut expected);
        super::super::append_add_r14_r10(&mut expected);
        super::super::append_mov_r15_imm64(&mut expected, 0);
        super::super::for_each_runtime_copy_chunk(36, 64, 4, |offset, chunk_size| {
            super::super::append_load_rax_from_r14(&mut expected, 36 + offset, chunk_size)?;
            super::super::append_store_rax_to_r15(&mut expected, 64 + offset, chunk_size)?;
            Ok(())
        })
        .expect("chunks");
        assert_eq!(bytes, expected);
        // Two base sites only: same-region indices record no index site.
        let sides: Vec<PlaceCopySide> = sites.iter().map(|(_, side)| side).collect();
        assert_eq!(sides, vec![PlaceCopySide::Source, PlaceCopySide::Target]);
    }

    /// Cross-region DOUBLE index: both index slots live in the FRAME while
    /// the place bases in the machine region -- each index materializes its
    /// own region base (r11 then r10) as a recorded site (SourceIndex then
    /// SourceIndex2) and loads through itself.
    #[test]
    fn double_index_cross_region_records_both_sites() {
        let source = Place::at(RuntimeStorageRegion::Machine, 32)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 8,
                index_byte_size: 4,
                element_byte_size: 16,
            })
            .unwrap()
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 12,
                index_byte_size: 4,
                element_byte_size: 4,
            })
            .unwrap();
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 64);
        let (bytes, sites) = encode_copy_places(&source, &target, 4).expect("encodes");

        let mut expected = Vec::new();
        super::super::append_mov_r14_imm64(&mut expected, 0);
        super::super::append_mov_r11_imm64(&mut expected, 0);
        super::super::append_load_unsigned_r11_from_r11(&mut expected, 8, 4).expect("first index");
        super::super::append_imul_r11_imm32(&mut expected, 16);
        super::super::append_mov_r10_imm64(&mut expected, 0);
        super::super::append_load_unsigned_r10_from_r10(&mut expected, 12, 4)
            .expect("second index");
        super::super::append_imul_r10_imm32(&mut expected, 4);
        super::super::append_add_r14_r11(&mut expected);
        super::super::append_add_r14_r10(&mut expected);
        super::super::append_mov_r15_imm64(&mut expected, 0);
        super::super::for_each_runtime_copy_chunk(32, 64, 4, |offset, chunk_size| {
            super::super::append_load_rax_from_r14(&mut expected, 32 + offset, chunk_size)?;
            super::super::append_store_rax_to_r15(&mut expected, 64 + offset, chunk_size)?;
            Ok(())
        })
        .expect("chunks");
        assert_eq!(bytes, expected);
        let sides: Vec<PlaceCopySide> = sites.iter().map(|(_, side)| side).collect();
        assert_eq!(
            sides,
            vec![
                PlaceCopySide::Source,
                PlaceCopySide::SourceIndex,
                PlaceCopySide::SourceIndex2,
                PlaceCopySide::Target,
            ]
        );
    }

    /// Three runtime indices refuse loudly: r11 and r10 are the only index
    /// scratches.
    #[test]
    fn triple_index_refuses() {
        let mut source = Place::at(RuntimeStorageRegion::Machine, 32);
        for offset in [8usize, 12, 16] {
            source = source
                .with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::Machine,
                    index_offset: offset,
                    index_byte_size: 4,
                    element_byte_size: 4,
                })
                .unwrap();
        }
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 64);
        let error = encode_copy_places(&source, &target, 4).expect_err("refuses");
        assert!(
            error
                .to_string()
                .contains("at most two runtime scaled indices")
        );
    }

    /// Write rung 1a: a DIRECT place target is byte-for-byte the retired
    /// integer-write layout (mov r15,imm64; mov rax,imm64; width store).
    #[test]
    fn place_integer_write_direct_matches_the_retired_layout() {
        let target = Place::at(RuntimeStorageRegion::Machine, 24);
        let (bytes, sites) = encode_place_integer_write(&target, 70, 4).expect("encodes");

        let mut expected = Vec::new();
        super::super::append_mov_r15_imm64(&mut expected, 0);
        super::super::append_mov_rax_imm64(&mut expected, 70);
        super::super::append_store_rax_to_r15(&mut expected, 24, 4).expect("store");
        assert_eq!(bytes, expected);
        let sides: Vec<PlaceCopySide> = sites.iter().map(|(_, side)| side).collect();
        assert_eq!(sides, vec![PlaceCopySide::Target]);
    }

    /// An INDEXED target rides the same index discipline: the index preloads
    /// into r11 while r15 still equals the region base, the add fires at the
    /// step position, the residual const folds into the store displacement.
    #[test]
    fn place_integer_write_indexed_layout() {
        let target = Place::at(RuntimeStorageRegion::Machine, 32)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 8,
                index_byte_size: 4,
                element_byte_size: 4,
            })
            .unwrap()
            .with_step(PlaceStep::ConstOffset(2))
            .unwrap();
        let (bytes, _) = encode_place_integer_write(&target, 9, 1).expect("encodes");

        let mut expected = Vec::new();
        super::super::append_mov_r15_imm64(&mut expected, 0);
        super::super::append_load_unsigned_reg_from_r15(
            &mut expected,
            super::super::Reg64::R11,
            8,
            4,
        )
        .expect("index");
        super::super::append_imul_r11_imm32(&mut expected, 4);
        super::super::append_add_r15_r11(&mut expected);
        super::super::append_mov_rax_imm64(&mut expected, 9);
        super::super::append_store_rax_to_r15(&mut expected, 34, 1).expect("store");
        assert_eq!(bytes, expected);
    }

    /// The selected index slot width reaches the actual load. Narrow slots
    /// zero-extend only their own bytes; a u64 index keeps all high bits.
    #[test]
    fn indexed_place_loads_exact_declared_width() {
        for (index_byte_size, opcode) in [
            (1usize, &[0x45, 0x0f, 0xb6, 0x9f][..]),
            (2, &[0x45, 0x0f, 0xb7, 0x9f][..]),
            (4, &[0x45, 0x8b, 0x9f][..]),
            (8, &[0x4d, 0x8b, 0x9f][..]),
        ] {
            let target = Place::at(RuntimeStorageRegion::Machine, 32)
                .with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::Machine,
                    index_offset: 8,
                    index_byte_size,
                    element_byte_size: 4,
                })
                .unwrap();
            let (bytes, _) = encode_place_integer_write(&target, 9, 1).expect("encodes");
            assert_eq!(
                &bytes[10..10 + opcode.len()],
                opcode,
                "index width {index_byte_size}"
            );
        }
    }

    #[test]
    fn general_text_assembly_walks_two_cross_region_target_indices() {
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 8,
                index_byte_size: 4,
                element_byte_size: 24,
            })
            .unwrap()
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 12,
                index_byte_size: 4,
                element_byte_size: 8,
            })
            .unwrap();

        let assert_sites = |sites: PlaceCopySites| {
            let sides = sites.iter().map(|(_, side)| side).collect::<Vec<_>>();
            assert_eq!(
                sides,
                vec![
                    PlaceCopySide::Target,
                    PlaceCopySide::TargetIndex,
                    PlaceCopySide::TargetIndex2,
                ]
            );
        };

        let (materialize, sites, buffer_site) =
            encode_place_text_buffer_materialize(&target).expect("materialize general target");
        assert_sites(sites);
        assert_eq!(&materialize[buffer_site..buffer_site + 2], &[0x49, 0xbe]);

        let (literal, sites, buffer_site) =
            encode_place_text_literal_append(&target, b"ok").expect("append literal");
        assert_sites(sites);
        assert_eq!(&literal[buffer_site..buffer_site + 2], &[0x49, 0xbe]);

        let (stored, sites, buffer_site, source_site) =
            encode_place_text_stored_append(&target, 48).expect("append stored source");
        assert_sites(sites);
        assert_eq!(&stored[buffer_site..buffer_site + 2], &[0x49, 0xbe]);
        assert_eq!(&stored[source_site..source_site + 2], &[0x48, 0xb9]);
    }

    #[test]
    fn runtime_frame_data_address_write_owns_one_word_and_two_bases() {
        let (bytes, sites) =
            encode_runtime_frame_data_address_write(40).expect("direct data-address write");
        assert_eq!(bytes.len(), 27);
        assert_eq!(
            sites.iter().collect::<Vec<_>>(),
            vec![(10, PlaceCopySide::Target)]
        );
        assert_eq!(&bytes[..2], &[0x49, 0xbe], "r14 owns the data relocation");
        assert_eq!(
            &bytes[10..12],
            &[0x49, 0xbf],
            "r15 owns the runtime-frame relocation"
        );
    }

    #[test]
    fn callback_function_address_store_preserves_exact_storage_region_and_sites() {
        for region in [
            RuntimeStorageRegion::Machine,
            RuntimeStorageRegion::RuntimeFrame,
        ] {
            let (bytes, sites) = encode_runtime_storage_function_address_write(region, 40).unwrap();
            assert_eq!(bytes.len(), 27);
            assert_eq!(
                sites.iter().collect::<Vec<_>>(),
                vec![(10, PlaceCopySide::Target)]
            );
            assert_eq!(&bytes[2..10], &[0; 8]);
            assert_eq!(&bytes[12..20], &[0; 8]);
        }
    }
}

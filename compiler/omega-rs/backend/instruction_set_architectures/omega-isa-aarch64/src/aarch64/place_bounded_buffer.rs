use omega_calling_conventions::{MachineRegister, MachineStateSet, RegisterSet};
use omega_target_operations::{Place, PlaceStep};
use psi_diagnostics::Diagnostic;

use super::primitives::{
    append_add_x_constant, append_unsigned_immediate, encode_add_page_offset_placeholder,
    encode_add_x_immediate, encode_add_x_register, encode_adrp_placeholder, encode_cbz_x,
    encode_load_byte_w_post_increment, encode_load_x_from_x, encode_store_byte_w_post_increment,
    encode_store_x_to_x, encode_subs_x_immediate, encode_unconditional_branch,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundedBufferPlaceSide {
    Target,
    Source,
}

#[derive(Debug, Clone, Default)]
pub struct BoundedBufferPlaceSites(Vec<(usize, BoundedBufferPlaceSide)>);

impl BoundedBufferPlaceSites {
    pub fn iter(&self) -> impl Iterator<Item = (usize, BoundedBufferPlaceSide)> + '_ {
        self.0.iter().copied()
    }
}

fn materialize_direct_or_pointee(
    bytes: &mut Vec<u8>,
    sites: &mut BoundedBufferPlaceSites,
    place: &Place,
    register: u8,
    side: BoundedBufferPlaceSide,
) -> Result<usize, Diagnostic> {
    sites.0.push((bytes.len(), side));
    bytes.extend(encode_adrp_placeholder(register));
    bytes.extend(encode_add_page_offset_placeholder(register));
    let mut displacement = 0usize;
    for step in place.steps() {
        match step {
            PlaceStep::ConstOffset(offset) => displacement += offset,
            PlaceStep::Deref => {
                bytes.extend(encode_load_x_from_x(register, register, displacement)?);
                displacement = 0;
            }
            PlaceStep::ScaledIndex { .. } => {
                return Err(Diagnostic::error(
                    "AArch64 bounded-buffer append supports direct and pointee Places; indexed append awaits the common AArch64 Place materializer",
                ));
            }
        }
    }
    Ok(displacement)
}

pub fn encode_place_bounded_buffer_source_append(
    target: &Place,
    source: &Place,
) -> Result<(Vec<u8>, BoundedBufferPlaceSites), Diagnostic> {
    let mut bytes = Vec::new();
    let mut sites = BoundedBufferPlaceSites::default();
    let target_offset = materialize_direct_or_pointee(
        &mut bytes,
        &mut sites,
        target,
        16,
        BoundedBufferPlaceSide::Target,
    )?;
    let source_offset = materialize_direct_or_pointee(
        &mut bytes,
        &mut sites,
        source,
        14,
        BoundedBufferPlaceSide::Source,
    )?;
    append_bounded_buffer_source(&mut bytes, target_offset, source_offset)?;
    Ok((bytes, sites))
}

fn append_bounded_buffer_source(
    bytes: &mut Vec<u8>,
    target_offset: usize,
    source_offset: usize,
) -> Result<(), Diagnostic> {
    bytes.extend(encode_load_x_from_x(15, 16, target_offset)?);
    bytes.extend(encode_load_x_from_x(13, 14, source_offset)?);
    append_add_x_constant(bytes, 12, 14, source_offset + 8, 10)?;
    append_add_x_constant(bytes, 11, 16, target_offset + 8, 10)?;
    bytes.extend(encode_add_x_register(11, 11, 15));
    bytes.extend(encode_add_x_register(15, 15, 13));
    bytes.extend(encode_store_x_to_x(15, 16, target_offset)?);
    bytes.extend(encode_cbz_x(13, 20)?);
    bytes.extend(encode_load_byte_w_post_increment(17, 12, 1)?);
    bytes.extend(encode_store_byte_w_post_increment(17, 11, 1)?);
    bytes.extend(encode_subs_x_immediate(13, 13, 1)?);
    bytes.extend(encode_unconditional_branch(-16)?);
    Ok(())
}

/// Append a direct/pointee source carrier after an indexed target recipe has
/// already materialized the exact destination carrier address into x16.
pub(super) fn append_bounded_buffer_source_to_x16(
    bytes: &mut Vec<u8>,
    source: &Place,
) -> Result<BoundedBufferPlaceSites, Diagnostic> {
    let mut sites = BoundedBufferPlaceSites::default();
    let source_offset = materialize_direct_or_pointee(
        bytes,
        &mut sites,
        source,
        14,
        BoundedBufferPlaceSide::Source,
    )?;
    append_bounded_buffer_source(bytes, 0, source_offset)?;
    Ok(sites)
}

pub fn place_bounded_buffer_source_append_register_write_ceiling() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(10),
        MachineRegister::Aarch64X(11),
        MachineRegister::Aarch64X(12),
        MachineRegister::Aarch64X(13),
        MachineRegister::Aarch64X(14),
        MachineRegister::Aarch64X(15),
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(26),
    ])
}

pub fn place_bounded_buffer_source_append_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([omega_calling_conventions::MachineState::Flags])
}

pub fn encode_place_bounded_buffer_literal_append(
    target: &Place,
    literal: &[u8],
) -> Result<(Vec<u8>, BoundedBufferPlaceSites), Diagnostic> {
    let mut bytes = Vec::new();
    let mut sites = BoundedBufferPlaceSites::default();
    let target_offset = materialize_direct_or_pointee(
        &mut bytes,
        &mut sites,
        target,
        16,
        BoundedBufferPlaceSide::Target,
    )?;
    bytes.extend(encode_load_x_from_x(15, 16, target_offset)?);
    append_add_x_constant(&mut bytes, 14, 16, target_offset + 8, 13)?;
    bytes.extend(encode_add_x_register(14, 14, 15));
    for byte in literal {
        append_unsigned_immediate(&mut bytes, 17, u64::from(*byte));
        bytes.extend(encode_store_byte_w_post_increment(17, 14, 1)?);
    }
    bytes.extend(encode_add_x_immediate(15, 15, literal.len())?);
    bytes.extend(encode_store_x_to_x(15, 16, target_offset)?);
    Ok((bytes, sites))
}

pub fn place_bounded_buffer_literal_append_register_write_ceiling() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(13),
        MachineRegister::Aarch64X(14),
        MachineRegister::Aarch64X(15),
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(26),
    ])
}

pub const fn place_bounded_buffer_literal_append_additional_machine_state() -> MachineStateSet {
    MachineStateSet::empty()
}

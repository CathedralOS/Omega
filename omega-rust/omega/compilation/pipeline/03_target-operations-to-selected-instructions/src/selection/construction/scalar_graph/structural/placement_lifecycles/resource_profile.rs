//! Read and validate the selected provider's complete runtime profile.
//!
//! The lifecycle owner authenticates the core carrier and the selected call.
//! This module derives byte geometry from that retained declaration and emits
//! bounded loops over its actual value. It neither trusts an ordinary Boolean
//! from source nor reconstructs a profile from placement demand. Validation is
//! distinct from compatibility and backing authority; malformed data must not
//! become an ordinary IncompatibleSupply result at the establishing consumer.

use super::super::super::operation_control;
use super::{
    Builder, ImageHome, LegalizedScalarInstruction, SelectedInstructionError,
    SelectedInstructionKind, VirtualRegisterId, failure, fragmented_scalar,
};

pub(super) mod compatibility;
mod geometry;
#[cfg(test)]
mod tests;
use geometry::{Counted, ProfileLayout, Scalar, TransferLayout};

/// Scratch contains only operation-local counters and an accumulated predicate.
/// It must be disjoint from the immutable profile and any provider backing.
pub(super) const VALIDATION_SCRATCH_BYTES: u32 = 48;
const VALID: u32 = 0;
const REGION: u32 = 8;
const PRIOR_REGION: u32 = 16;
const TRANSFER: u32 = 24;
const PRIOR_TRANSFER: u32 = 32;
const SERVICE: u32 = 40;

#[derive(Clone, Copy)]
struct Emission<'row> {
    row: &'row LegalizedScalarInstruction,
    profile: ImageHome<'row>,
    profile_bytes: u32,
    scratch: ImageHome<'row>,
}

impl Emission<'_> {
    fn constant(
        self,
        builder: &mut Builder<'_>,
        value: u64,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        fragmented_scalar::constant(builder, self.row, self.scratch.place, value)
    }

    fn binary(
        self,
        builder: &mut Builder<'_>,
        kind: SelectedInstructionKind,
        left: VirtualRegisterId,
        right: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        fragmented_scalar::binary(builder, self.row, self.scratch.place, kind, left, right)
    }

    fn compare(
        self,
        builder: &mut Builder<'_>,
        kind: SelectedInstructionKind,
        left: VirtualRegisterId,
        right: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        super::outcome::compare(builder, self.row, self.scratch.place, left, right, kind)
    }

    fn equal(
        self,
        builder: &mut Builder<'_>,
        left: VirtualRegisterId,
        right: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        self.compare(
            builder,
            SelectedInstructionKind::MaterializeBooleanEqual,
            left,
            right,
        )
    }

    fn at_most(
        self,
        builder: &mut Builder<'_>,
        left: VirtualRegisterId,
        right: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        self.compare(
            builder,
            SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
            left,
            right,
        )
    }

    fn invert(
        self,
        builder: &mut Builder<'_>,
        value: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let one = self.constant(builder, 1)?;
        self.binary(builder, SelectedInstructionKind::BitwiseXorI64, value, one)
    }

    fn nonzero(
        self,
        builder: &mut Builder<'_>,
        value: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let zero = self.constant(builder, 0)?;
        let absent = self.equal(builder, value, zero)?;
        self.invert(builder, absent)
    }

    fn read_scratch(
        self,
        builder: &mut Builder<'_>,
        byte_offset: u32,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        super::ScalarImageCodec::Bytes {
            byte_offset,
            width: 8,
            signed: false,
        }
        .read(builder, self.row, self.scratch)
    }

    fn write_scratch(
        self,
        builder: &mut Builder<'_>,
        byte_offset: u32,
        value: VirtualRegisterId,
    ) -> Result<(), SelectedInstructionError> {
        super::ScalarImageCodec::Bytes {
            byte_offset,
            width: 8,
            signed: false,
        }
        .write(builder, self.row, self.scratch, value)
    }

    fn require(
        self,
        builder: &mut Builder<'_>,
        predicate: VirtualRegisterId,
    ) -> Result<(), SelectedInstructionError> {
        let valid = self.read_scratch(builder, VALID)?;
        let valid = self.binary(
            builder,
            SelectedInstructionKind::BitwiseAndI64,
            valid,
            predicate,
        )?;
        self.write_scratch(builder, VALID, valid)
    }

    /// Dynamic indexing is bounded by the counted carrier before any load.
    /// Attribute its conservative complete read footprint to the actual profile
    /// root, not a fabricated source index or a scratch address.
    fn load(
        self,
        builder: &mut Builder<'_>,
        pointer: VirtualRegisterId,
        scalar: Scalar,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let mut value = self.constant(builder, 0)?;
        let displacement = self.constant(builder, u64::from(scalar.offset))?;
        let address = self.binary(
            builder,
            SelectedInstructionKind::WrappingAddI64,
            pointer,
            displacement,
        )?;
        for byte in 0..scalar.width {
            let output = super::super::transport_register(builder, self.profile.place, 0)?;
            self.profile.storage.footprint(
                builder,
                self.row,
                self.profile.place,
                self.profile.byte_offset,
                self.profile_bytes,
                super::super::runtime_address::Written::None,
            )?;
            builder.emit(
                SelectedInstructionKind::Load8 { byte_offset: byte },
                builder
                    .constraints
                    .keys
                    .load8
                    .ok_or_else(|| failure("profile validation target lacks byte loads"))?,
                &[address, output],
                super::super::provenance(self.row),
            )?;
            let shifted = fragmented_scalar::shift(
                builder,
                self.row,
                self.scratch.place,
                output,
                u64::from(byte) * 8,
                true,
            )?;
            value = self.binary(
                builder,
                SelectedInstructionKind::BitwiseOrI64,
                value,
                shifted,
            )?;
        }
        Ok(value)
    }

    fn element(
        self,
        builder: &mut Builder<'_>,
        pointer: VirtualRegisterId,
        array: Counted,
        position: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let stride = self.constant(builder, u64::from(array.stride))?;
        let displacement = self.binary(
            builder,
            SelectedInstructionKind::WrappingMultiplyI64,
            position,
            stride,
        )?;
        let offset = self.constant(builder, u64::from(array.offset))?;
        let displacement = self.binary(
            builder,
            SelectedInstructionKind::WrappingAddI64,
            displacement,
            offset,
        )?;
        self.binary(
            builder,
            SelectedInstructionKind::WrappingAddI64,
            pointer,
            displacement,
        )
    }

    /// An invalid count contributes false, then executes zero iterations. It
    /// never licenses an out-of-range read while the remaining checks run.
    fn bounded_count(
        self,
        builder: &mut Builder<'_>,
        pointer: VirtualRegisterId,
        array: Counted,
        nonempty: bool,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let count = self.load(builder, pointer, array.count)?;
        let capacity = self.constant(builder, array.capacity)?;
        let bounded = self.at_most(builder, count, capacity)?;
        self.require(builder, bounded)?;
        if nonempty {
            let present = self.nonzero(builder, count)?;
            self.require(builder, present)?;
        }
        self.binary(
            builder,
            SelectedInstructionKind::WrappingMultiplyI64,
            count,
            bounded,
        )
    }

    fn repeat(
        self,
        builder: &mut Builder<'_>,
        count: VirtualRegisterId,
        counter: u32,
        body: impl FnOnce(&mut Builder<'_>, VirtualRegisterId) -> Result<(), SelectedInstructionError>,
    ) -> Result<(), SelectedInstructionError> {
        let zero = self.constant(builder, 0)?;
        self.write_scratch(builder, counter, zero)?;
        operation_control::while_nonzero(
            builder,
            self.row,
            |builder| {
                let position = self.read_scratch(builder, counter)?;
                let finished = self.at_most(builder, count, position)?;
                self.invert(builder, finished)
            },
            |builder| {
                let position = self.read_scratch(builder, counter)?;
                body(builder, position)?;
                let one = self.constant(builder, 1)?;
                let next = self.binary(
                    builder,
                    SelectedInstructionKind::WrappingAddI64,
                    position,
                    one,
                )?;
                self.write_scratch(builder, counter, next)
            },
        )
    }

    fn transfer_rules(
        self,
        builder: &mut Builder<'_>,
        pointer: VirtualRegisterId,
        array: Counted,
        layout: &TransferLayout,
    ) -> Result<(), SelectedInstructionError> {
        let count = self.bounded_count(builder, pointer, array, true)?;
        self.repeat(builder, count, TRANSFER, |builder, position| {
            let entry = self.element(builder, pointer, array, position)?;
            let width = self.load(builder, entry, layout.width)?;
            let alignment = self.load(builder, entry, layout.alignment)?;
            let present = self.nonzero(builder, width)?;
            self.require(builder, present)?;
            let maximum = self.constant(builder, 128)?;
            let bounded = self.at_most(builder, width, maximum)?;
            self.require(builder, bounded)?;
            let byte_mask = self.constant(builder, 7)?;
            let remainder = self.binary(
                builder,
                SelectedInstructionKind::BitwiseAndI64,
                width,
                byte_mask,
            )?;
            let zero = self.constant(builder, 0)?;
            let whole_bytes = self.equal(builder, remainder, zero)?;
            self.require(builder, whole_bytes)?;
            let present = self.nonzero(builder, alignment)?;
            self.require(builder, present)?;
            let one = self.constant(builder, 1)?;
            let previous = self.binary(
                builder,
                SelectedInstructionKind::WrappingSubtractI64,
                alignment,
                one,
            )?;
            let bits = self.binary(
                builder,
                SelectedInstructionKind::BitwiseAndI64,
                alignment,
                previous,
            )?;
            let power = self.equal(builder, bits, zero)?;
            self.require(builder, power)?;
            if let Some(operations) = &layout.operations {
                let mut any = zero;
                for operation in operations {
                    let value = self.load(builder, entry, *operation)?;
                    let boolean = self.at_most(builder, value, one)?;
                    self.require(builder, boolean)?;
                    any =
                        self.binary(builder, SelectedInstructionKind::BitwiseOrI64, any, value)?;
                }
                let present = self.nonzero(builder, any)?;
                self.require(builder, present)?;
            }
            self.repeat(
                builder,
                position,
                PRIOR_TRANSFER,
                |builder, prior_position| {
                    let prior = self.element(builder, pointer, array, prior_position)?;
                    let prior_width = self.load(builder, prior, layout.width)?;
                    let duplicate = self.equal(builder, width, prior_width)?;
                    let distinct = self.invert(builder, duplicate)?;
                    self.require(builder, distinct)
                },
            )
        })
    }
}

/// Return only well-formedness, not compatibility, admission, or permission to
/// commit bytes. Every active region and capability participates, even if the
/// requested placement would never access that region.
#[cfg(test)]
fn validate(
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    profile_type: semantic_vocabulary::StructuralTypeId,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    profile: ImageHome,
    scratch: ImageHome,
    available: VirtualRegisterId,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let layout = ProfileLayout::derive(declarations, profile_type)?;
    validate_layout(&layout, builder, row, profile, scratch, available)
}

fn validate_layout(
    layout: &ProfileLayout,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    profile: ImageHome,
    scratch: ImageHome,
    available: VirtualRegisterId,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    super::offset(scratch.byte_offset, u64::from(VALIDATION_SCRATCH_BYTES))?;
    super::offset(profile.byte_offset, u64::from(layout.bytes))?;
    let emission = Emission {
        row,
        profile,
        profile_bytes: layout.bytes,
        scratch,
    };
    let valid = emission.nonzero(builder, available)?;
    emission.write_scratch(builder, VALID, valid)?;
    let displacement = emission.constant(builder, u64::from(profile.byte_offset))?;
    let pointer = emission.binary(
        builder,
        SelectedInstructionKind::WrappingAddI64,
        profile.pointer,
        displacement,
    )?;
    let count = emission.bounded_count(builder, pointer, layout.regions, false)?;
    emission.repeat(builder, count, REGION, |builder, position| {
        let region = emission.element(builder, pointer, layout.regions, position)?;
        let start = emission.load(builder, region, layout.start)?;
        let length = emission.load(builder, region, layout.length)?;
        let end = emission.binary(
            builder,
            SelectedInstructionKind::WrappingAddI64,
            start,
            length,
        )?;
        let nonempty = emission.nonzero(builder, length)?;
        emission.require(builder, nonempty)?;
        let nonoverflow = emission.at_most(builder, start, end)?;
        emission.require(builder, nonoverflow)?;
        let contained = emission.at_most(builder, end, available)?;
        emission.require(builder, contained)?;
        emission.repeat(
            builder,
            position,
            PRIOR_REGION,
            |builder, prior_position| {
                let prior = emission.element(builder, pointer, layout.regions, prior_position)?;
                let prior_start = emission.load(builder, prior, layout.start)?;
                let prior_length = emission.load(builder, prior, layout.length)?;
                let prior_end = emission.binary(
                    builder,
                    SelectedInstructionKind::WrappingAddI64,
                    prior_start,
                    prior_length,
                )?;
                let before = emission.at_most(builder, end, prior_start)?;
                let after = emission.at_most(builder, prior_end, start)?;
                let disjoint = emission.binary(
                    builder,
                    SelectedInstructionKind::BitwiseOrI64,
                    before,
                    after,
                )?;
                emission.require(builder, disjoint)
            },
        )?;
        let peer = emission.load(builder, region, layout.peer)?;
        let stable = emission.load(builder, region, layout.stable)?;
        let external = emission.load(builder, region, layout.external)?;
        let atomic = emission.load(builder, region, layout.atomic)?;
        let one = emission.constant(builder, 1)?;
        let three = emission.constant(builder, 3)?;
        for tag in [peer, external, atomic] {
            let known = emission.at_most(builder, tag, one)?;
            emission.require(builder, known)?;
        }
        let known = emission.at_most(builder, stable, three)?;
        emission.require(builder, known)?;
        let stable_present = emission.nonzero(builder, stable)?;
        let hostile_stable = emission.binary(
            builder,
            SelectedInstructionKind::BitwiseAndI64,
            peer,
            stable_present,
        )?;
        let safe = emission.invert(builder, hostile_stable)?;
        emission.require(builder, safe)?;
        let any = emission.binary(
            builder,
            SelectedInstructionKind::BitwiseOrI64,
            stable,
            external,
        )?;
        let any = emission.binary(builder, SelectedInstructionKind::BitwiseOrI64, any, atomic)?;
        let present = emission.nonzero(builder, any)?;
        emission.require(builder, present)?;
        let external_active = emission.equal(builder, external, one)?;
        operation_control::when_nonzero(builder, row, external_active, |builder| {
            let read = emission.load(builder, region, layout.external_read)?;
            let write = emission.load(builder, region, layout.external_write)?;
            let two = emission.constant(builder, 2)?;
            let known = emission.at_most(builder, read, two)?;
            emission.require(builder, known)?;
            let boolean = emission.at_most(builder, write, one)?;
            emission.require(builder, boolean)?;
            let any =
                emission.binary(builder, SelectedInstructionKind::BitwiseOrI64, read, write)?;
            let present = emission.nonzero(builder, any)?;
            emission.require(builder, present)?;
            emission.transfer_rules(
                builder,
                region,
                layout.external_rules,
                &layout.external_transfer,
            )
        })?;
        let atomic_active = emission.equal(builder, atomic, one)?;
        operation_control::when_nonzero(builder, row, atomic_active, |builder| {
            emission.transfer_rules(
                builder,
                region,
                layout.atomic_rules,
                &layout.atomic_transfer,
            )
        })?;
        let services = emission.bounded_count(builder, region, layout.reach, false)?;
        emission.repeat(builder, services, SERVICE, |builder, service_position| {
            let service_pointer =
                emission.element(builder, region, layout.reach, service_position)?;
            let service = emission.load(builder, service_pointer, layout.service)?;
            let valid = emission.nonzero(builder, service)?;
            emission.require(builder, valid)
        })
    })?;
    emission.read_scratch(builder, VALID)
}

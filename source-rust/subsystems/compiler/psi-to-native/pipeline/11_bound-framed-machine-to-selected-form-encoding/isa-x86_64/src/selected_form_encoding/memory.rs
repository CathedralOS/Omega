use super::emit::{modrm, rex, rex64, rr64};
use super::request_validation::{guard_canonical, resolve_registers};
use super::{
    MachineAlternativeFamily as Family, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect as MemEff, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior as Trap, RegisterViewId, SelectedInstructionKind as Kind,
    ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
    X86_64SelectedFormEncodingError as Error, X86_64SelectedFormFootprint,
};
use selected_instructions::FrameStorageSlotId;

pub(super) mod frame {
    use super::*;

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<(RegisterViewId, Option<RegisterViewId>, u32), Error> {
        if matches!(
            kind,
            Kind::Store64 {
                slot: FrameStorageSlotId::ActivationBase,
                ..
            }
        ) || matches!(kind, Kind::FrameAddress { slot: FrameStorageSlotId::ActivationBase, byte_offset } if byte_offset != 0)
        {
            return Err(Error::EncodedFormMismatch);
        }
        let (family, store, member) = match kind {
            Kind::Store64 { byte_offset, .. } => (Family::Store64, true, byte_offset),
            Kind::FrameAddress { byte_offset, .. } => (Family::FrameAddress, false, byte_offset),
            _ => return Err(Error::AlternativeMismatch),
        };
        if physical.identity() != crate::canonical_x86_64_physical_register_model_identity()
            || alternative != (MachineAlternativeKey { family, variant: 0 })
        {
            return Err(Error::AlternativeMismatch);
        }
        resolve_registers(physical, operands)?;
        match *operands {
            [value, scratch] if store && value != scratch => Ok((scratch, Some(value), member)),
            [address] if !store => Ok((address, None, member)),
            _ => Err(Error::OperandCountMismatch),
        }
    }

    fn check_activation_base(
        kind: Kind,
        displacement: i64,
        alignment_bytes: u16,
    ) -> Result<(), Error> {
        if matches!(
            kind,
            Kind::FrameAddress {
                slot: FrameStorageSlotId::ActivationBase,
                ..
            }
        ) && (displacement != 0 || alignment_bytes != 1)
        {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(())
    }

    fn finish(
        physical: &ValidatedPhysicalRegisterModel,
        address: RegisterViewId,
        value: Option<RegisterViewId>,
        alignment_bytes: u16,
        bytes: Vec<u8>,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let store = value.is_some();
        let catalog = crate::x86_64_register_constraint_catalog_for(physical);
        let key = if store {
            crate::X86_64_STORE64
        } else {
            crate::X86_64_FRAME_ADDRESS
        };
        let constraint = catalog.constraint(key).ok_or(Error::AlternativeMismatch)?;
        let stack_pointer = physical
            .model()
            .view_named("rsp")
            .ok_or(Error::EncodedFormMismatch)?
            .id;
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes,
            footprint: X86_64SelectedFormFootprint {
                register_reads: value.into_iter().collect(),
                register_writes: vec![address],
                writes_rflags: alignment_bytes != 1,
                encoded: MachineEncodedEffects {
                    external_operand_reads: if store { vec![0] } else { Vec::new() },
                    external_operand_writes: vec![u16::from(store)],
                    implicit_unit_uses: constraint.implicit_uses.clone(),
                    implicit_unit_defs: constraint.implicit_defs.clone(),
                    implicit_unit_clobbers: constraint.clobbers.clone(),
                    memory: if store {
                        MemEff::WriteFrameStorageV1 {
                            stack_pointer,
                            byte_count: 8,
                        }
                    } else {
                        MemEff::NoneV1
                    },
                    stack: MachineEncodedStackEffect::UnchangedV1,
                    trap: if store {
                        Trap::MayArchitecturalFaultV1
                    } else {
                        Trap::NeverV1
                    },
                    control: MachineEncodedControlEffect::FallThroughV1,
                },
            },
        })
    }

    pub fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
        alignment_bytes: u16,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        check_activation_base(kind, displacement, alignment_bytes)?;
        let (address, value, member) = request(physical, kind, alternative, operands)?;
        let disp = i32::try_from(displacement).map_err(|_| Error::MemoryDisplacementOutsideI32)?;
        let mut bytes = if alignment_bytes == 1 {
            crate::frame_protocol::encode_frame_address_signed(physical, address, disp)
                .map_err(|_| Error::EncodedFormMismatch)?
        } else {
            let offset = u32::try_from(disp).map_err(|_| Error::EncodedFormMismatch)?;
            crate::encode_x86_64_aligned_frame_address(
                physical,
                address,
                offset,
                alignment_bytes,
                member,
            )
            .map_err(|_| Error::EncodedFormMismatch)?
        };
        if let Some(value) = value {
            bytes.extend_from_slice(
                &crate::encode_x86_64_pointer_memory(physical, value, address, 0, 8, false)
                    .map_err(|_| Error::EncodedFormMismatch)?,
            );
        }
        finish(physical, address, value, alignment_bytes, bytes)
    }

    pub fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
        alignment_bytes: u16,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        check_activation_base(kind, displacement, alignment_bytes)?;
        let (address, value, member) = request(physical, kind, alternative, operands)?;
        let disp = i32::try_from(displacement).map_err(|_| Error::MemoryDisplacementOutsideI32)?;
        let addr_bytes = if let Some(value) = value {
            crate::frame_protocol::decode_pointer_store_suffix(physical, value, address, bytes)
                .ok_or(Error::EncodedFormMismatch)?
        } else {
            bytes
        };
        if !crate::frame_protocol::validate_frame_address(
            physical,
            address,
            disp,
            alignment_bytes,
            member,
            addr_bytes,
        ) {
            return Err(Error::EncodedFormMismatch);
        }
        finish(physical, address, value, alignment_bytes, bytes.to_vec())
    }
}

mod indexed {
    use super::*;

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<(u16, [u8; 3]), Error> {
        guard_canonical(physical)?;
        let (family, width) = match kind {
            Kind::Load8Indexed => (Family::Load8Indexed, 1u16),
            Kind::Load16Indexed => (Family::Load16Indexed, 2),
            Kind::Load32Indexed => (Family::Load32Indexed, 4),
            Kind::Load64Indexed => (Family::Load64Indexed, 8),
            _ => return Err(Error::EncodedFormMismatch),
        };
        if displacement != 0 || alternative != (MachineAlternativeKey { family, variant: 0 }) {
            return Err(Error::EncodedFormMismatch);
        }
        let regs: [u8; 3] = resolve_registers(physical, operands)?
            .try_into()
            .map_err(|_| Error::EncodedFormMismatch)?;
        if regs[1] == 4 {
            return Err(Error::EncodedFormMismatch);
        }
        Ok((width, regs))
    }

    fn encode_bytes(width: u16, [base, index, dst]: [u8; 3]) -> Vec<u8> {
        let mut bytes = vec![rex(width != 4, dst, index, base)];
        match width {
            1 => bytes.extend([0x0f, 0xb6]),
            2 => bytes.extend([0x0f, 0xb7]),
            _ => bytes.push(0x8b),
        }
        let mode = if width == 1 {
            2
        } else {
            u8::from(base & 7 == 5)
        };
        bytes.push(modrm(mode, dst, 4));
        bytes.push(((width.trailing_zeros() as u8) << 6) | ((index & 7) << 3) | (base & 7));
        if mode == 1 {
            bytes.push(0);
        } else if mode == 2 {
            bytes.extend([0; 4]);
        }
        bytes
    }

    fn finish(
        width: u16,
        operands: &[RegisterViewId],
        bytes: Vec<u8>,
    ) -> ValidatedX86_64SelectedFormEncoding {
        ValidatedX86_64SelectedFormEncoding {
            bytes,
            footprint: X86_64SelectedFormFootprint {
                register_reads: vec![operands[0], operands[1]],
                register_writes: vec![operands[2]],
                writes_rflags: false,
                encoded: MachineEncodedEffects {
                    external_operand_reads: vec![0, 1],
                    external_operand_writes: vec![2],
                    implicit_unit_uses: Vec::new(),
                    implicit_unit_defs: Vec::new(),
                    implicit_unit_clobbers: Vec::new(),
                    memory: if width == 1 {
                        MemEff::ReadIndexedPointerV1 {
                            pointer_operand: 0,
                            index_operand: 1,
                            byte_count: 1,
                        }
                    } else {
                        MemEff::ReadScaledIndexV1 {
                            pointer_operand: 0,
                            index_operand: 1,
                            element_bytes: width,
                        }
                    },
                    stack: MachineEncodedStackEffect::UnchangedV1,
                    trap: Trap::MayArchitecturalFaultV1,
                    control: MachineEncodedControlEffect::FallThroughV1,
                },
            },
        }
    }

    pub(super) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let (width, regs) = request(physical, kind, alternative, operands, displacement)?;
        Ok(finish(width, operands, encode_bytes(width, regs)))
    }

    pub(super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let (width, regs) = request(physical, kind, alternative, operands, displacement)?;
        if bytes.is_empty() {
            return Err(Error::MalformedEncoding);
        }
        if bytes != encode_bytes(width, regs) {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(finish(width, operands, bytes.to_vec()))
    }
}

mod packed {
    use super::*;

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<(bool, u8, u32, [u8; 3], X86_64SelectedFormFootprint), Error> {
        guard_canonical(physical)?;
        let disp = u32::try_from(displacement).map_err(|_| Error::EncodedFormMismatch)?;
        let (load, width, offset) = match kind {
            Kind::LoadPacked { byte_offset, width } => (true, width.byte_size(), byte_offset),
            Kind::StorePacked { byte_offset, width } => (false, width.byte_size(), byte_offset),
            _ => return Err(Error::EncodedFormMismatch),
        };
        let family = if load {
            match width {
                3 => Family::LoadPacked3,
                5 => Family::LoadPacked5,
                6 => Family::LoadPacked6,
                7 => Family::LoadPacked7,
                _ => return Err(Error::EncodedFormMismatch),
            }
        } else {
            Family::StorePacked
        };
        if alternative != (MachineAlternativeKey { family, variant: 0 }) {
            return Err(Error::AlternativeMismatch);
        }
        if offset != disp
            || disp
                .checked_add(u32::from(width) - 1)
                .is_none_or(|end| end > i32::MAX as u32)
        {
            return Err(Error::EncodedFormMismatch);
        }
        let regs: [u8; 3] = resolve_registers(physical, operands)?
            .try_into()
            .map_err(|_| Error::OperandCountMismatch)?;
        if regs[2] == regs[0] || regs[2] == regs[1] || (load && regs[0] == regs[1]) {
            return Err(Error::EncodedFormMismatch);
        }
        let reads = if load { vec![0u16] } else { vec![0, 1] };
        let writes = if load { vec![1u16, 2] } else { vec![2] };
        let flags = physical
            .model()
            .view_named("condition_flags")
            .ok_or(Error::NonCanonicalPhysicalModel)?;
        let footprint = X86_64SelectedFormFootprint {
            register_reads: reads.iter().map(|&i| operands[i as usize]).collect(),
            register_writes: writes.iter().map(|&i| operands[i as usize]).collect(),
            writes_rflags: true,
            encoded: MachineEncodedEffects {
                external_operand_reads: reads,
                external_operand_writes: writes,
                implicit_unit_uses: Vec::new(),
                implicit_unit_defs: Vec::new(),
                implicit_unit_clobbers: flags.units.clone(),
                memory: if load {
                    MemEff::ReadPointerV1 {
                        pointer_operand: 0,
                        byte_count: u16::from(width),
                    }
                } else {
                    MemEff::WritePointerV1 { pointer_operand: 0 }
                },
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: Trap::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::FallThroughV1,
            },
        };
        Ok((load, width, disp, regs, footprint))
    }

    fn encode_bytes(load: bool, width: u8, disp: u32, [base, value, scratch]: [u8; 3]) -> Vec<u8> {
        let mut bytes = Vec::new();
        if !load {
            rr64(&mut bytes, 0x89, value, scratch);
        }
        for i in 0..width {
            let reg = if load && i == 0 { value } else { scratch };
            bytes.push(rex(load, reg, 0, base));
            if load {
                bytes.push(0x0f);
            }
            bytes.extend([if load { 0xb6 } else { 0x88 }, modrm(2, reg, base)]);
            if base & 7 == 4 {
                bytes.push(0x24);
            }
            bytes.extend((disp + u32::from(i)).to_le_bytes());
            if load && i != 0 {
                bytes.extend([rex64(0, 0, scratch), 0xc1, modrm(3, 4, scratch), i * 8]);
                rr64(&mut bytes, 0x09, scratch, value);
            } else if !load && i + 1 != width {
                bytes.extend([rex64(0, 0, scratch), 0xc1, modrm(3, 5, scratch), 8]);
            }
        }
        bytes
    }

    pub(super) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let (load, width, disp, regs, footprint) =
            request(physical, kind, alternative, operands, displacement)?;
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: encode_bytes(load, width, disp, regs),
            footprint,
        })
    }

    pub(super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let (load, width, disp, regs, footprint) =
            request(physical, kind, alternative, operands, displacement)?;
        if bytes != encode_bytes(load, width, disp, regs) {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint,
        })
    }
}

fn accessor_scalar_target(
    kind: Kind,
    alternative: MachineAlternativeKey,
) -> Result<Option<(Kind, MachineAlternativeKey)>, Error> {
    let Kind::AccessorScalarAccess {
        form, byte_offset, ..
    } = kind
    else {
        return Ok(None);
    };
    if alternative
        != (MachineAlternativeKey {
            family: Family::AccessorScalarAccess(form),
            variant: 0,
        })
    {
        return Err(Error::EncodedFormMismatch);
    }
    let ordinary = form
        .ordinary_kind(byte_offset)
        .ok_or(Error::EncodedFormMismatch)?;
    let family = form
        .ordinary_semantic()
        .ok_or(Error::EncodedFormMismatch)?
        .into();
    Ok(Some((
        ordinary,
        MachineAlternativeKey { family, variant: 0 },
    )))
}

fn store_width(kind: Kind) -> Result<u8, Error> {
    match kind {
        Kind::Store {
            byte_size: w @ (1 | 2 | 4 | 8),
            ..
        } => Ok(w),
        Kind::Store { .. } => Err(Error::EncodedFormMismatch),
        Kind::Load8 { .. } => Ok(1),
        Kind::Load16 { .. } => Ok(2),
        Kind::Load32 { .. } => Ok(4),
        _ => Ok(8),
    }
}

fn ordinary_request(
    physical: &ValidatedPhysicalRegisterModel,
    kind: Kind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: i64,
) -> Result<(Vec<u8>, X86_64SelectedFormFootprint), Error> {
    if matches!(
        kind,
        Kind::Store64 {
            slot: FrameStorageSlotId::Incoming { .. },
            ..
        }
    ) {
        return Err(Error::EncodedFormMismatch);
    }
    guard_canonical(physical)?;
    let width = store_width(kind)?;
    let (family, opcode, key) = match kind {
        Kind::Store { .. } => (
            Family::Store,
            if width == 1 { 0x88 } else { 0x89 },
            crate::X86_64_STORE,
        ),
        Kind::AddressOffset { .. } => (Family::AddressOffset, 0x8d, crate::X86_64_ADDRESS_OFFSET),
        Kind::Load8 { .. } => (Family::Load8, 0xb6, crate::X86_64_LOAD8),
        Kind::Load16 { .. } => (Family::Load16, 0xb7, crate::X86_64_LOAD16),
        Kind::Load32 { .. } => (Family::Load32, 0x8b, crate::X86_64_LOAD32),
        Kind::Load64 { .. } => (Family::Load64, 0x8b, crate::X86_64_LOAD64),
        _ => return Err(Error::AlternativeMismatch),
    };
    if alternative != (MachineAlternativeKey { family, variant: 0 }) {
        return Err(Error::AlternativeMismatch);
    }
    if operands.len() != 2 {
        return Err(Error::OperandCountMismatch);
    }
    let regs = resolve_registers(physical, operands)?;
    let (base, reg) = (regs[0], regs[1]);
    let disp = i32::try_from(displacement).map_err(|_| Error::MemoryDisplacementOutsideI32)?;
    if matches!(
        kind,
        selected_instructions::load_width_triple_pattern!(byte_offset, ..)
            | Kind::Load64 { byte_offset }
            | Kind::Store { byte_offset, .. }
            | Kind::AddressOffset { byte_offset }
            if i64::from(byte_offset) != displacement
    ) {
        return Err(Error::EncodedFormMismatch);
    }
    let narrow_load = matches!(kind, Kind::Load8 { .. } | Kind::Load16 { .. });
    let mut bytes = Vec::with_capacity(9);
    if width == 2 && !narrow_load {
        bytes.push(0x66);
    }
    bytes.push(rex(width == 8 || narrow_load, reg, 0, base));
    if narrow_load {
        bytes.push(0x0f);
    }
    bytes.extend([opcode, modrm(2, reg, base)]);
    if base & 7 == 4 {
        bytes.push(0x24);
    }
    bytes.extend(disp.to_le_bytes());

    let catalog = crate::x86_64_register_constraint_catalog_for(physical);
    let constraint = catalog
        .constraint(key)
        .expect("canonical memory constraint");
    let (reads, writes, memory, trap) = match kind {
        Kind::Store { .. } => (
            vec![0u16, 1],
            vec![],
            MemEff::WritePointerV1 { pointer_operand: 0 },
            Trap::MayArchitecturalFaultV1,
        ),
        Kind::AddressOffset { .. } => (vec![0], vec![1], MemEff::NoneV1, Trap::NeverV1),
        _ => (
            vec![0],
            vec![1],
            MemEff::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: u16::from(width),
            },
            Trap::MayArchitecturalFaultV1,
        ),
    };
    let footprint = X86_64SelectedFormFootprint {
        register_reads: reads.iter().map(|&i| operands[i as usize]).collect(),
        register_writes: writes.iter().map(|&i| operands[i as usize]).collect(),
        writes_rflags: false,
        encoded: MachineEncodedEffects {
            external_operand_reads: reads,
            external_operand_writes: writes,
            implicit_unit_uses: constraint.implicit_uses.clone(),
            implicit_unit_defs: constraint.implicit_defs.clone(),
            implicit_unit_clobbers: constraint.clobbers.clone(),
            memory,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap,
            control: MachineEncodedControlEffect::FallThroughV1,
        },
    };
    Ok((bytes, footprint))
}

pub fn encode_x86_64_selected_memory_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: Kind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: i64,
) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
    if let Some((ord, alt)) = accessor_scalar_target(kind, alternative)? {
        return encode_x86_64_selected_memory_form(physical, ord, alt, operands, displacement);
    }
    if matches!(kind, Kind::Store64 { .. } | Kind::FrameAddress { .. }) {
        return frame::encode(physical, kind, alternative, operands, displacement, 1);
    }
    if matches!(
        kind,
        Kind::SaveFloatingControl { .. } | Kind::RestoreFloatingControl { .. }
    ) {
        return super::floating_control::encode_x86_64_selected_floating_control_form(
            physical,
            kind,
            alternative,
            operands,
            displacement,
        );
    }
    if matches!(kind, Kind::LoadPacked { .. } | Kind::StorePacked { .. }) {
        return packed::encode(physical, kind, alternative, operands, displacement);
    }
    if kind.indexed_load_bytes().is_some() {
        return indexed::encode(physical, kind, alternative, operands, displacement);
    }
    let (bytes, footprint) = ordinary_request(physical, kind, alternative, operands, displacement)?;
    Ok(ValidatedX86_64SelectedFormEncoding { bytes, footprint })
}

pub fn validate_x86_64_selected_memory_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: Kind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: i64,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
    if let Some((ord, alt)) = accessor_scalar_target(kind, alternative)? {
        return validate_x86_64_selected_memory_form(
            physical,
            ord,
            alt,
            operands,
            displacement,
            bytes,
        );
    }
    if matches!(kind, Kind::Store64 { .. } | Kind::FrameAddress { .. }) {
        return frame::validate(
            physical,
            kind,
            alternative,
            operands,
            displacement,
            1,
            bytes,
        );
    }
    if matches!(
        kind,
        Kind::SaveFloatingControl { .. } | Kind::RestoreFloatingControl { .. }
    ) {
        return super::floating_control::validate_x86_64_selected_floating_control_form(
            physical,
            kind,
            alternative,
            operands,
            displacement,
            bytes,
        );
    }
    if matches!(kind, Kind::LoadPacked { .. } | Kind::StorePacked { .. }) {
        return packed::validate(physical, kind, alternative, operands, displacement, bytes);
    }
    if kind.indexed_load_bytes().is_some() {
        return indexed::validate(physical, kind, alternative, operands, displacement, bytes);
    }
    let (expected, footprint) =
        ordinary_request(physical, kind, alternative, operands, displacement)?;
    if bytes.len() != expected.len() {
        return Err(Error::MalformedEncoding);
    }
    if bytes != expected {
        return Err(Error::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint,
    })
}

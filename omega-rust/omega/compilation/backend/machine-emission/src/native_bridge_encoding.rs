//! Physical register, pointer and frame transfers shared by native adapters.
//!
//! These operations encode already-admitted placements. They grant no source
//! ownership, reference backing, domain membership or foreign-call authority.
//! Each adapter owns its incoming/outgoing signature and snapshots values before
//! distributing them; this leaf owns their byte transfers and aligned homes.

use calling_conventions::{
    IndirectPointerLocation, MachineRegister, ValueClass, ValueLocation, ValuePlacement,
};
use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use target::Architecture;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeBridgeEncodingError {
    UnsupportedValueTransport,
    FrameOverflow,
    Encoding,
}
type Error = NativeBridgeEncodingError;

#[derive(Clone, Copy)]
pub(crate) enum WordLocation {
    Register(MachineRegister),
    Stack(u32),
}

#[derive(Clone, Copy)]
pub(crate) struct ValuePiece {
    pub(crate) location: WordLocation,
    pub(crate) value_byte_offset: u32,
    pub(crate) byte_size: u8,
}

pub(crate) enum ValueStorage {
    Direct(Vec<ValuePiece>),
    Indirect {
        pointer: ValuePiece,
        copy_offset: Option<u32>,
    },
}

impl ValueStorage {
    pub(crate) fn pieces(&self) -> &[ValuePiece] {
        match self {
            Self::Direct(pieces) => pieces,
            Self::Indirect { pointer, .. } => std::slice::from_ref(pointer),
        }
    }
    pub(crate) fn is_indirect(&self) -> bool {
        matches!(self, Self::Indirect { .. })
    }
}

pub(crate) fn pointer_piece(pointer: IndirectPointerLocation) -> ValuePiece {
    ValuePiece {
        location: match pointer {
            IndirectPointerLocation::Register(register) => WordLocation::Register(register),
            IndirectPointerLocation::Stack {
                stack_byte_offset, ..
            } => WordLocation::Stack(stack_byte_offset),
        },
        value_byte_offset: 0,
        byte_size: 8,
    }
}

pub(crate) fn value_storage(
    placement: &ValuePlacement,
    byte_size: u32,
) -> Result<ValueStorage, Error> {
    if !is_borrowed(placement.shape.class)
        && let [
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size: extent,
                alignment,
            },
        ] = placement.locations.as_slice()
    {
        if *extent != byte_size || !alignment.is_power_of_two() {
            return Err(Error::UnsupportedValueTransport);
        }
        return Ok(ValueStorage::Indirect {
            pointer: pointer_piece(*pointer),
            copy_offset: *copy_stack_byte_offset,
        });
    }
    Ok(ValueStorage::Direct(value_pieces(placement, byte_size)?))
}

pub(crate) fn is_borrowed(class: ValueClass) -> bool {
    matches!(
        class,
        ValueClass::BorrowedReference | ValueClass::BorrowedScalarReference
    )
}

/// Physical pieces partition one complete byte extent. ABI class selects the
/// banks, not a different value: owned integer/HFA/SysV records may redistribute
/// the same admitted storage across different registers. Loan geometry instead
/// measures a referent or descriptor, and is normalized only to its pointer.
pub(crate) fn value_pieces(
    placement: &ValuePlacement,
    byte_size: u32,
) -> Result<Vec<ValuePiece>, Error> {
    let mut pieces = Vec::new();
    for location in &placement.locations {
        match *location {
            ValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } if matches!(byte_size, 1 | 2 | 4 | 8) => {
                pieces.push(ValuePiece {
                    location: WordLocation::Register(register),
                    value_byte_offset,
                    byte_size: byte_size as u8,
                });
            }
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset,
                byte_size,
                ..
            } => {
                let mut remaining = byte_size;
                let mut copied = 0_u32;
                while remaining != 0 {
                    let width = [8, 4, 2, 1]
                        .into_iter()
                        .find(|width| *width <= remaining)
                        .ok_or(Error::UnsupportedValueTransport)?;
                    pieces.push(ValuePiece {
                        location: WordLocation::Stack(
                            stack_byte_offset
                                .checked_add(copied)
                                .ok_or(Error::FrameOverflow)?,
                        ),
                        value_byte_offset: value_byte_offset
                            .checked_add(copied)
                            .ok_or(Error::FrameOverflow)?,
                        byte_size: width as u8,
                    });
                    copied = copied.checked_add(width).ok_or(Error::FrameOverflow)?;
                    remaining -= width;
                }
            }
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset: None,
                ..
            } if is_borrowed(placement.shape.class) => {
                pieces.push(ValuePiece {
                    location: match pointer {
                        IndirectPointerLocation::Register(register) => {
                            WordLocation::Register(register)
                        }
                        IndirectPointerLocation::Stack {
                            stack_byte_offset, ..
                        } => WordLocation::Stack(stack_byte_offset),
                    },
                    value_byte_offset: 0,
                    byte_size: 8,
                });
            }
            _ => return Err(Error::UnsupportedValueTransport),
        }
    }
    // Plans are already admitted. This partition check makes this encoder's
    // extent assumption explicit and prevents reading padding past an extent.
    pieces.sort_by_key(|piece| piece.value_byte_offset);
    let mut end = 0_u32;
    for piece in &pieces {
        if piece.value_byte_offset != end {
            return Err(Error::UnsupportedValueTransport);
        }
        end = end
            .checked_add(u32::from(piece.byte_size))
            .ok_or(Error::FrameOverflow)?;
    }
    if end != byte_size {
        return Err(Error::UnsupportedValueTransport);
    }
    Ok(pieces)
}

pub(crate) fn allocate_value(cursor: &mut u32, byte_size: u32) -> Result<u32, Error> {
    *cursor = cursor
        .checked_add(7)
        .map(|value| value & !7)
        .ok_or(Error::FrameOverflow)?;
    let offset = *cursor;
    *cursor = cursor.checked_add(byte_size).ok_or(Error::FrameOverflow)?;
    Ok(offset)
}

pub(crate) fn allocate_word(cursor: &mut u32) -> Result<u32, Error> {
    allocate_value(cursor, 8)
}

pub(crate) struct Encoding {
    pub(crate) architecture: Architecture,
    pub(crate) model: ValidatedPhysicalRegisterModel,
    pub(crate) scratch: MachineRegister,
    pub(crate) address_scratch: MachineRegister,
    pub(crate) aligned_homes: Vec<AlignedHome>,
}

pub(crate) struct AlignedHome {
    pub(crate) offset: u32,
    pub(crate) byte_size: u32,
    pub(crate) alignment: u16,
}

pub(crate) fn allocate_home(
    cursor: &mut u32,
    byte_size: u32,
    alignment: u16,
    homes: &mut Vec<AlignedHome>,
) -> Result<u32, Error> {
    if !alignment.is_power_of_two() {
        return Err(Error::UnsupportedValueTransport);
    }
    let base_alignment = u32::from(alignment.clamp(8, 16));
    *cursor = cursor
        .checked_add(base_alignment - 1)
        .map(|offset| offset & !(base_alignment - 1))
        .ok_or(Error::FrameOverflow)?;
    let offset = *cursor;
    *cursor = cursor
        .checked_add(byte_size)
        .and_then(|end| end.checked_add(u32::from(alignment.saturating_sub(16))))
        .ok_or(Error::FrameOverflow)?;
    if alignment > 16 {
        homes.push(AlignedHome {
            offset,
            byte_size,
            alignment,
        });
    }
    Ok(offset)
}

impl Encoding {
    pub(crate) fn view(&self, register: MachineRegister) -> Result<RegisterViewId, Error> {
        let name = match register {
            MachineRegister::Aarch64X(number) if number < 31 => format!("x{number}"),
            MachineRegister::Aarch64V(number) if number < 32 => format!("d{number}"),
            MachineRegister::X86Xmm(number) if number < 16 => format!("xmm{number}"),
            register if register.architecture() == Architecture::X86_64 => {
                let names = [
                    "rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", "r8", "r9", "r10",
                    "r11", "r12", "r13", "r14", "r15",
                ];
                let name = names
                    .get(usize::from(register.codec_index()))
                    .ok_or(Error::Encoding)?;
                if *name == "rsp" {
                    return Err(Error::Encoding);
                }
                (*name).to_owned()
            }
            _ => return Err(Error::Encoding),
        };
        self.model
            .model()
            .view_named(&name)
            .map(|view| view.id)
            .ok_or(Error::Encoding)
    }

    pub(crate) fn memory(
        &self,
        bytes: &mut Vec<u8>,
        register: MachineRegister,
        offset: u32,
        size: u8,
        load: bool,
    ) -> Result<(), Error> {
        let view = if size == 16
            && let MachineRegister::Aarch64V(number) = register
        {
            self.model
                .model()
                .view_named(&format!("q{number}"))
                .map(|view| view.id)
                .ok_or(Error::Encoding)?
        } else {
            self.view(register)?
        };
        if let Some(home) = self
            .aligned_homes
            .iter()
            .find(|home| offset >= home.offset && offset - home.offset < home.byte_size)
        {
            if offset - home.offset + u32::from(size) > home.byte_size
                || (!load && register == self.address_scratch)
            {
                return Err(Error::Encoding);
            }
            self.address_register(bytes, self.address_scratch, offset)?;
            let base = self.view(self.address_scratch)?;
            let encoded = match self.architecture {
                Architecture::Aarch64 => isa_aarch64::encode_aarch64_pointer_memory(
                    &self.model,
                    view,
                    base,
                    0,
                    size,
                    load,
                )
                .map_err(|_| Error::Encoding)?,
                Architecture::X86_64 => {
                    isa_x86_64::encode_x86_64_pointer_memory(&self.model, view, base, 0, size, load)
                        .map_err(|_| Error::Encoding)?
                }
            };
            bytes.extend_from_slice(&encoded);
            return Ok(());
        }
        let encoded = match self.architecture {
            Architecture::Aarch64 => isa_aarch64::encode_aarch64_frame_memory(
                &self.model,
                view,
                offset,
                size,
                load,
                self.view(self.address_scratch)?,
            )
            .map_err(|_| Error::Encoding)?,
            Architecture::X86_64 => {
                isa_x86_64::encode_x86_64_frame_memory(&self.model, view, offset, size, load)
                    .map_err(|_| Error::Encoding)?
            }
        };
        bytes.extend_from_slice(&encoded);
        Ok(())
    }

    pub(crate) fn address(&self, bytes: &mut Vec<u8>, offset: u32) -> Result<(), Error> {
        self.address_register(bytes, self.scratch, offset)
    }

    pub(crate) fn address_register(
        &self,
        bytes: &mut Vec<u8>,
        register: MachineRegister,
        offset: u32,
    ) -> Result<(), Error> {
        let view = self.view(register)?;
        if let Some(home) = self
            .aligned_homes
            .iter()
            .find(|home| offset >= home.offset && offset - home.offset < home.byte_size)
        {
            let encoded = match self.architecture {
                Architecture::Aarch64 => isa_aarch64::encode_aarch64_aligned_frame_address(
                    &self.model,
                    view,
                    home.offset,
                    home.alignment,
                    offset - home.offset,
                )
                .map_err(|_| Error::Encoding)?,
                Architecture::X86_64 => isa_x86_64::encode_x86_64_aligned_frame_address(
                    &self.model,
                    view,
                    home.offset,
                    home.alignment,
                    offset - home.offset,
                )
                .map_err(|_| Error::Encoding)?,
            };
            bytes.extend_from_slice(&encoded);
            return Ok(());
        }
        let encoded = match self.architecture {
            Architecture::Aarch64 => {
                isa_aarch64::encode_aarch64_frame_address(&self.model, view, offset)
                    .map_err(|_| Error::Encoding)?
            }
            Architecture::X86_64 => {
                isa_x86_64::encode_x86_64_frame_address(&self.model, view, offset)
                    .map_err(|_| Error::Encoding)?
            }
        };
        bytes.extend_from_slice(&encoded);
        Ok(())
    }

    pub(crate) fn constant(&self, bytes: &mut Vec<u8>, value: u64) -> Result<(), Error> {
        let view = self.view(self.scratch)?;
        let encoded = match self.architecture {
            Architecture::Aarch64 => {
                isa_aarch64::encode_aarch64_integer_constant(&self.model, view, value)
                    .map_err(|_| Error::Encoding)?
            }
            Architecture::X86_64 => {
                isa_x86_64::encode_x86_64_integer_constant(&self.model, view, value)
                    .map_err(|_| Error::Encoding)?
            }
        };
        bytes.extend_from_slice(&encoded);
        Ok(())
    }

    pub(crate) fn place_address(
        &self,
        bytes: &mut Vec<u8>,
        location: WordLocation,
        offset: u32,
    ) -> Result<(), Error> {
        match location {
            WordLocation::Register(register) => self.address_register(bytes, register, offset),
            WordLocation::Stack(destination) => {
                self.address(bytes, offset)?;
                self.memory(bytes, self.scratch, destination, 8, false)
            }
        }
    }

    pub(crate) fn pointer_memory(
        &self,
        bytes: &mut Vec<u8>,
        offset: u32,
        size: u8,
        load: bool,
    ) -> Result<(), Error> {
        let view = self.view(self.scratch)?;
        let base = self.view(self.address_scratch)?;
        let encoded = match self.architecture {
            Architecture::Aarch64 => isa_aarch64::encode_aarch64_pointer_memory(
                &self.model,
                view,
                base,
                offset,
                size,
                load,
            )
            .map_err(|_| Error::Encoding)?,
            Architecture::X86_64 => isa_x86_64::encode_x86_64_pointer_memory(
                &self.model,
                view,
                base,
                offset,
                size,
                load,
            )
            .map_err(|_| Error::Encoding)?,
        };
        bytes.extend_from_slice(&encoded);
        Ok(())
    }

    pub(crate) fn copy_indirect(
        &self,
        bytes: &mut Vec<u8>,
        snapshot: u32,
        pointer: u32,
        extent: u32,
        load: bool,
    ) -> Result<(), Error> {
        let mut offset = 0_u32;
        while offset < extent {
            let size = [8, 4, 2, 1]
                .into_iter()
                .find(|size| *size <= extent - offset)
                .ok_or(Error::UnsupportedValueTransport)?;
            let slot = snapshot.checked_add(offset).ok_or(Error::FrameOverflow)?;
            // Frame addressing may reuse the base scratch. Reload the saved
            // pointer after every frame operation, including large offsets.
            if !load {
                self.memory(bytes, self.scratch, slot, size as u8, true)?;
            }
            self.memory(bytes, self.address_scratch, pointer, 8, true)?;
            self.pointer_memory(bytes, offset, size as u8, load)?;
            if load {
                self.memory(bytes, self.scratch, slot, size as u8, false)?;
            }
            offset = offset.checked_add(size).ok_or(Error::FrameOverflow)?;
        }
        Ok(())
    }

    pub(crate) fn distribute_argument(
        &self,
        bytes: &mut Vec<u8>,
        storage: &ValueStorage,
        snapshot: u32,
        extent: u32,
    ) -> Result<(), Error> {
        match storage {
            ValueStorage::Direct(pieces) => self.distribute(bytes, pieces, snapshot),
            ValueStorage::Indirect {
                pointer,
                copy_offset,
            } => {
                let destination = if let Some(destination) = copy_offset {
                    let mut offset = 0;
                    while offset < extent {
                        let size = [8, 4, 2, 1]
                            .into_iter()
                            .find(|size| *size <= extent - offset)
                            .ok_or(Error::UnsupportedValueTransport)?;
                        self.copy_word(
                            bytes,
                            snapshot.checked_add(offset).ok_or(Error::FrameOverflow)?,
                            destination
                                .checked_add(offset)
                                .ok_or(Error::FrameOverflow)?,
                            size as u8,
                        )?;
                        offset = offset.checked_add(size).ok_or(Error::FrameOverflow)?;
                    }
                    *destination
                } else {
                    snapshot
                };
                self.place_address(bytes, pointer.location, destination)
            }
        }
    }

    pub(crate) fn capture_registers(
        &self,
        bytes: &mut Vec<u8>,
        pieces: &[ValuePiece],
        snapshot: u32,
    ) -> Result<(), Error> {
        for piece in pieces {
            if let WordLocation::Register(register) = piece.location {
                self.memory(
                    bytes,
                    register,
                    snapshot
                        .checked_add(piece.value_byte_offset)
                        .ok_or(Error::FrameOverflow)?,
                    piece.byte_size,
                    false,
                )?;
            }
        }
        Ok(())
    }

    pub(crate) fn capture_stack(
        &self,
        bytes: &mut Vec<u8>,
        pieces: &[ValuePiece],
        snapshot: u32,
        bias: u32,
    ) -> Result<(), Error> {
        for piece in pieces {
            if let WordLocation::Stack(offset) = piece.location {
                self.copy_word(
                    bytes,
                    bias.checked_add(offset).ok_or(Error::FrameOverflow)?,
                    snapshot
                        .checked_add(piece.value_byte_offset)
                        .ok_or(Error::FrameOverflow)?,
                    piece.byte_size,
                )?;
            }
        }
        Ok(())
    }

    pub(crate) fn distribute(
        &self,
        bytes: &mut Vec<u8>,
        pieces: &[ValuePiece],
        snapshot: u32,
    ) -> Result<(), Error> {
        // Stack stores use excluded scratches; register reloads then restore
        // every fixed destination without consuming a partly rewritten value.
        for piece in pieces {
            if let WordLocation::Stack(offset) = piece.location {
                self.copy_word(
                    bytes,
                    snapshot
                        .checked_add(piece.value_byte_offset)
                        .ok_or(Error::FrameOverflow)?,
                    offset,
                    piece.byte_size,
                )?;
            }
        }
        for piece in pieces {
            if let WordLocation::Register(register) = piece.location {
                self.memory(
                    bytes,
                    register,
                    snapshot
                        .checked_add(piece.value_byte_offset)
                        .ok_or(Error::FrameOverflow)?,
                    piece.byte_size,
                    true,
                )?;
            }
        }
        Ok(())
    }

    pub(crate) fn copy_word(
        &self,
        bytes: &mut Vec<u8>,
        source: u32,
        destination: u32,
        size: u8,
    ) -> Result<(), Error> {
        self.memory(bytes, self.scratch, source, size, true)?;
        self.memory(bytes, self.scratch, destination, size, false)
    }
}

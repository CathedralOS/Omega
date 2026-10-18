//! Unresolved image addresses, independent of their referent's ownership.
//!
//! Selection names an exact referent, and the encoded instruction materializes
//! its address PC-relatively with the address field left zero. That field
//! travels unchanged through layout,
//! fragment emission, frame application and text placement, translated only
//! from row- to function- to section-relative coordinates; object
//! construction binds it to the owning symbol. Literals and implementation
//! tables have different placement owners, not different relocation mechanics.
//! An address observation itself supplies neither a borrow nor storage custody.

use selected_instructions::{SelectedBlockId, SelectedInstructionId};
use semantic_vocabulary::{MachineId, OperationId, PlaceId};

/// The Terminal byte literal whose octets an address names: the establishing
/// operation and the literal place it declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteLiteralReference {
    pub operation: OperationId,
    pub place: PlaceId,
}

/// The unresolved field(s) of one image address materialization.
/// Offsets are relative to whatever the containing record names: the
/// encoded row, the function, or the text section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageAddressFields {
    /// `lea reg, [rip + disp32]`; the disp32 is the field, and it ends the
    /// instruction, so its reference point is the next instruction.
    X86_64Relative32 { patch_offset: u64 },
    /// `adrp reg, page` then `add reg, reg, #lo12`; both immediates are fields.
    Aarch64PageAddress {
        page_patch_offset: u64,
        page_offset_patch_offset: u64,
    },
}

impl ImageAddressFields {
    /// The same fields after their containing span moves by `by` bytes.
    fn shifted_by(self, by: u64, offset: impl Fn(u64, u64) -> Option<u64>) -> Option<Self> {
        Some(match self {
            Self::X86_64Relative32 { patch_offset } => Self::X86_64Relative32 {
                patch_offset: offset(patch_offset, by)?,
            },
            Self::Aarch64PageAddress {
                page_patch_offset,
                page_offset_patch_offset,
            } => Self::Aarch64PageAddress {
                page_patch_offset: offset(page_patch_offset, by)?,
                page_offset_patch_offset: offset(page_offset_patch_offset, by)?,
            },
        })
    }

    /// The same fields shifted forward by `by` bytes.
    pub fn shifted(self, by: u64) -> Option<Self> {
        self.shifted_by(by, u64::checked_add)
    }

    /// The same fields relative to a span that starts `by` bytes later.
    pub fn unshifted(self, by: u64) -> Option<Self> {
        self.shifted_by(by, u64::checked_sub)
    }

    /// Canonical identity bytes shared by every record that hashes fields.
    pub fn encode_identity(self, bytes: &mut Vec<u8>) {
        match self {
            Self::X86_64Relative32 { patch_offset } => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, patch_offset.to_le_bytes());
            }
            Self::Aarch64PageAddress {
                page_patch_offset,
                page_offset_patch_offset,
            } => {
                bytes.push(2);
                crate::codec_bytes::push_encoded(bytes, page_patch_offset.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, page_offset_patch_offset.to_le_bytes());
            }
        }
    }
}

impl ByteLiteralReference {
    pub fn encode_identity(self, bytes: &mut Vec<u8>) {
        crate::codec_bytes::push_encoded(bytes, self.operation.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, self.place.get().to_le_bytes());
    }
}

/// Row-relative unresolved image address carried from encoding
/// through resolved layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedFormImageAddressFixup {
    pub reference: selected_instructions::SelectedImageAddressReference,
    pub fields: ImageAddressFields,
}

/// Function-relative unresolved image address inside one fragment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionFragmentImageAddressFixup {
    pub reference: selected_instructions::SelectedImageAddressReference,
    pub fields: ImageAddressFields,
}

/// Section-relative placement of one unresolved image address. The
/// instruction's bytes keep their zero field; object construction binds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacedImageAddress {
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub instruction: SelectedInstructionId,
    pub reference: selected_instructions::SelectedImageAddressReference,
    pub operation: OperationId,
    pub instruction_function_offset: u64,
    pub instruction_section_offset: u64,
    pub instruction_byte_count: u64,
    pub fields: ImageAddressFields,
}

impl PlacedImageAddress {
    pub fn byte_literal(&self) -> Option<ByteLiteralReference> {
        match self.reference {
            selected_instructions::SelectedImageAddressReference::ByteLiteral {
                operation,
                place,
            } => Some(ByteLiteralReference { operation, place }),
            _ => None,
        }
    }

    pub fn implementation_table(&self) -> Option<u32> {
        match self.reference {
            selected_instructions::SelectedImageAddressReference::ImplementationTable { table } => {
                Some(table)
            }
            _ => None,
        }
    }

    pub fn encode_identity(&self, bytes: &mut Vec<u8>) {
        crate::codec_bytes::push_encoded(bytes, self.machine.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, self.block.0.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, self.instruction.0.to_le_bytes());
        self.reference.encode_identity(bytes);
        crate::codec_bytes::push_encoded(bytes, self.operation.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, self.instruction_function_offset.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, self.instruction_section_offset.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, self.instruction_byte_count.to_le_bytes());
        self.fields.encode_identity(bytes);
    }
}

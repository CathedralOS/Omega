//! Unresolved addresses of byte literals' read-only image storage.
//!
//! A quoted literal's view denotes immutable image storage, not a slot in the
//! activation that establishes it. Selection therefore names the literal's
//! octets by the Terminal operation and place that establish them, and the
//! encoded instruction materializes their address PC-relatively with its
//! address field left zero. That field travels unchanged through layout,
//! fragment emission, frame application and text placement, translated only
//! from row- to function- to section-relative coordinates; object
//! construction binds it to the literal's read-only image symbol. No layer
//! patches it in place.

use selected_instructions::{SelectedBlockId, SelectedInstructionId};
use semantic_vocabulary::{MachineId, OperationId, PlaceId};

/// The Terminal byte literal whose octets an address names: the establishing
/// operation and the literal place it declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteLiteralReference {
    pub operation: OperationId,
    pub place: PlaceId,
}

/// The unresolved field(s) of one byte-literal address materialization.
/// Offsets are relative to whatever the containing record names: the
/// encoded row, the function, or the text section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteLiteralAddressFields {
    /// `lea reg, [rip + disp32]`; the disp32 is the field, and it ends the
    /// instruction, so its reference point is the next instruction.
    X86_64Relative32 { patch_offset: u64 },
    /// `adrp reg, page` then `add reg, reg, #lo12`; both immediates are fields.
    Aarch64PageAddress {
        page_patch_offset: u64,
        page_offset_patch_offset: u64,
    },
}

impl ByteLiteralAddressFields {
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

/// Row-relative unresolved byte-literal address carried from encoding
/// through resolved layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedFormByteLiteralAddressFixup {
    pub literal: ByteLiteralReference,
    pub fields: ByteLiteralAddressFields,
}

/// Function-relative unresolved byte-literal address inside one fragment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionFragmentByteLiteralAddressFixup {
    pub literal: ByteLiteralReference,
    pub fields: ByteLiteralAddressFields,
}

/// Section-relative placement of one unresolved byte-literal address. The
/// instruction's bytes keep their zero field; object construction binds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacedByteLiteralAddress {
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub instruction: SelectedInstructionId,
    pub literal: ByteLiteralReference,
    pub instruction_function_offset: u64,
    pub instruction_section_offset: u64,
    pub instruction_byte_count: u64,
    pub fields: ByteLiteralAddressFields,
}

impl PlacedByteLiteralAddress {
    pub fn encode_identity(&self, bytes: &mut Vec<u8>) {
        crate::codec_bytes::push_encoded(bytes, self.machine.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, self.block.0.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, self.instruction.0.to_le_bytes());
        self.literal.encode_identity(bytes);
        crate::codec_bytes::push_encoded(bytes, self.instruction_function_offset.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, self.instruction_section_offset.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, self.instruction_byte_count.to_le_bytes());
        self.fields.encode_identity(bytes);
    }
}

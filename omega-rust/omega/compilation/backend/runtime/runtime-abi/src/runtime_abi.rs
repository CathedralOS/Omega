//! One place that knows what a two-word descriptor looks like, so nothing else
//! open-codes `2 * pointer_size`.
//!
//! Every descriptor carrier falls out of a single number the target hands us:
//! `pointer_size`. A slice, a text window, and a borrowed dynamic-trait value
//! are all two pointer-sized words:
//!
//! ```text
//!   offset 0              offset pointer_size
//!   +---------------------+---------------------+
//!   | ptr                 | len                 |  slice: len counts elements
//!   | ptr                 | len                 |  text:  len counts bytes
//!   | instance            | table               |  dynamic trait: table pointer
//!   +---------------------+---------------------+
//! ```
//!
//! On a 64-bit target that is 16 bytes, 8-aligned, second word at offset 8.
//!
//! The three rows sharing one geometry is exactly why this crate owns only the
//! geometry. It does not model which carrier a descriptor describes: the second
//! word's meaning (element count, byte count, or selected-conformance table
//! pointer) belongs to the lowering that produces the descriptor, where
//! applying a slice operation to a dynamic value is a compile error rather than
//! a question for a layout type.
//!
//! `layout` is the only consumer of this geometry, through
//! `sizing.rs::fat_descriptor_layout`.

use target::NativeTarget;

/// Byte size and alignment of the two-word descriptor shape
/// `{ first word @ 0, second word @ pointer_size }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FatDescriptorLayout {
    pub byte_size: usize,
    pub alignment: usize,
}

/// The descriptor geometry shared by slice, text-window and dynamic-trait
/// carriers: two pointer-sized words, pointer-aligned.
pub const fn fat_descriptor_layout(target: NativeTarget) -> FatDescriptorLayout {
    FatDescriptorLayout {
        byte_size: target.pointer_size.saturating_mul(2),
        alignment: target.pointer_alignment,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use target::{Architecture, ObjectFormat};

    #[test]
    fn descriptor_geometry_tracks_the_target_pointer_words() {
        for pointer_size in [4, 8] {
            let target = NativeTarget {
                architecture: Architecture::X86_64,
                object_format: ObjectFormat::Coff,
                pointer_size,
                pointer_alignment: pointer_size,
            };
            let layout = fat_descriptor_layout(target);
            assert_eq!(layout.byte_size, pointer_size * 2);
            assert_eq!(layout.alignment, pointer_size);
        }
    }
}

//! Where an indexed field byte read finds its bytes. A bounded owner keeps
//! them inline after its live-length word; a shared view field keeps a
//! two-word descriptor whose first word points at backing outside the root.
//! Construction and replay share this geometry so neither re-derives it.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::selection) enum FieldBytes {
    /// The payload's offset from the root.
    Inline(u32),
    /// The descriptor's offset from the root; its pointer word is loaded.
    Descriptor(u32),
}

impl FieldBytes {
    pub(in crate::selection) const fn offset(self) -> u32 {
        match self {
            Self::Inline(offset) | Self::Descriptor(offset) => offset,
        }
    }
}

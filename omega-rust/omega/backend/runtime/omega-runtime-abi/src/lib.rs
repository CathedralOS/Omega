use omega_target::NativeTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeAbiPlan {
    pub pointer_size: usize,
    pub pointer_alignment: usize,
}

impl Default for RuntimeAbiPlan {
    fn default() -> Self {
        build_runtime_abi_plan(NativeTarget::host())
    }
}

/// Discriminates the two carriers that share the single fat-descriptor layout.
///
/// Both variants have byte-identical layout (`{ptr, len}`); they differ only in
/// the meaning of `len` (element count for `Slice`, byte count for `TextWindow`)
/// and thus in how consumers interpret the descriptor. Ownership (owned vs
/// borrowed) is NOT encoded here — it lives in the semantic spine and never
/// affects layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FatDescriptorKind {
    /// `len` counts elements; pointer addresses the first element.
    Slice,
    /// `len` counts bytes; pointer addresses the first byte of the window.
    TextWindow,
}

/// The single, canonical fat-descriptor shape `{ ptr @ 0, len @ pointer_size }`.
///
/// This is the ONE owner of the `{ptr,len}` layout used by both slices and text
/// windows. `omega-layout` and instruction-selection are CONSUMERS: they must
/// derive descriptor size, the length-half offset, and alignment from here
/// rather than re-deriving `+ pointer_size` / `2 * pointer_size` independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FatDescriptorAbi {
    pointer_size: usize,
    pointer_alignment: usize,
    kind: FatDescriptorKind,
}

/// Artifact-local borrowed dynamic-trait carrier `{ instance, table }`.
///
/// Its two words happen to have the same size and alignment as a slice
/// descriptor, but the second word is a selected-conformance table pointer,
/// never a length. Keeping a distinct ABI view prevents later lowering from
/// applying slice operations to dynamic values merely because their layouts
/// coincide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DynamicTraitDescriptorAbi {
    pointer_size: usize,
    pointer_alignment: usize,
}

/// Result of resolving a subslice against a base descriptor.
///
/// `ptr_delta` is the byte offset to add to the base pointer to reach the first
/// element of the subslice (`start * element_byte_size`). `len` is the new
/// element/byte count (`end - start`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubsliceLayout {
    pub ptr_delta: usize,
    pub len: usize,
}

impl FatDescriptorAbi {
    /// Build a fat-descriptor view of the given kind from a runtime ABI plan.
    pub const fn new(plan: RuntimeAbiPlan, kind: FatDescriptorKind) -> Self {
        Self {
            pointer_size: plan.pointer_size,
            pointer_alignment: plan.pointer_alignment,
            kind,
        }
    }

    /// Build a slice-carrier descriptor (`len` = element count).
    pub const fn slice(plan: RuntimeAbiPlan) -> Self {
        Self::new(plan, FatDescriptorKind::Slice)
    }

    /// Build a text-window descriptor (`len` = byte count).
    pub const fn text_window(plan: RuntimeAbiPlan) -> Self {
        Self::new(plan, FatDescriptorKind::TextWindow)
    }

    /// Which carrier this descriptor describes.
    pub const fn kind(self) -> FatDescriptorKind {
        self.kind
    }

    /// Byte offset of the pointer half. Always 0.
    pub const fn ptr_offset(self) -> usize {
        0
    }

    /// Byte offset of the length half. One pointer past the pointer half.
    pub const fn len_offset(self) -> usize {
        self.pointer_size
    }

    /// Byte width of the length half. One pointer wide.
    pub const fn len_size(self) -> usize {
        self.pointer_size
    }

    /// Total descriptor size. Two pointers wide.
    pub const fn total_size(self) -> usize {
        self.pointer_size.saturating_mul(2)
    }

    /// Descriptor alignment. Pointer-aligned.
    pub const fn align(self) -> usize {
        self.pointer_alignment
    }

    /// Resolve a subslice `[start, end)` against a base whose first element lives
    /// at `base_ptr_offset` with elements of `element_byte_size` bytes.
    ///
    /// Returns the byte delta to add to the base pointer and the new length.
    /// Subslicing is uniform across carriers: `new.ptr = base.ptr + start *
    /// element_byte_size`, `new.len = end - start`.
    pub fn subslice(
        self,
        base_ptr_offset: usize,
        element_byte_size: usize,
        start: usize,
        end: usize,
    ) -> Option<SubsliceLayout> {
        if start > end {
            return None;
        }
        let ptr_delta = start
            .checked_mul(element_byte_size)
            .and_then(|scaled| base_ptr_offset.checked_add(scaled))?;
        Some(SubsliceLayout {
            ptr_delta,
            len: end - start,
        })
    }
}

impl DynamicTraitDescriptorAbi {
    pub const fn new(plan: RuntimeAbiPlan) -> Self {
        Self {
            pointer_size: plan.pointer_size,
            pointer_alignment: plan.pointer_alignment,
        }
    }

    /// Byte offset of the borrowed concrete instance pointer.
    pub const fn instance_offset(self) -> usize {
        0
    }

    /// Byte offset of the artifact-private selected-conformance table pointer.
    pub const fn table_offset(self) -> usize {
        self.pointer_size
    }

    pub const fn word_size(self) -> usize {
        self.pointer_size
    }

    pub const fn total_size(self) -> usize {
        self.pointer_size.saturating_mul(2)
    }

    pub const fn align(self) -> usize {
        self.pointer_alignment
    }
}

impl RuntimeAbiPlan {
    /// Slice fat-descriptor view (`len` = element count).
    pub const fn slice_descriptor(self) -> FatDescriptorAbi {
        FatDescriptorAbi::slice(self)
    }

    /// Text-window fat-descriptor view (`len` = byte count).
    pub const fn text_descriptor(self) -> FatDescriptorAbi {
        FatDescriptorAbi::text_window(self)
    }

    /// Borrowed local dynamic-trait descriptor (`{ instance, table }`).
    pub const fn dynamic_trait_descriptor(self) -> DynamicTraitDescriptorAbi {
        DynamicTraitDescriptorAbi::new(self)
    }

    /// Thin shim retained for source compatibility; equals the canonical
    /// `FatDescriptorAbi::total_size()`.
    pub const fn string_descriptor_size(self) -> usize {
        self.text_descriptor().total_size()
    }

    /// Thin shim retained for source compatibility; equals the canonical
    /// `FatDescriptorAbi::total_size()`.
    pub const fn slice_descriptor_size(self) -> usize {
        self.slice_descriptor().total_size()
    }
}

pub const fn build_runtime_abi_plan(target: NativeTarget) -> RuntimeAbiPlan {
    RuntimeAbiPlan {
        pointer_size: target.pointer_size,
        pointer_alignment: target.pointer_alignment,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(pointer_size: usize) -> RuntimeAbiPlan {
        RuntimeAbiPlan {
            pointer_size,
            pointer_alignment: pointer_size,
        }
    }

    #[test]
    fn fat_descriptor_offsets_are_canonical() {
        let abi = plan(8).slice_descriptor();
        assert_eq!(abi.ptr_offset(), 0);
        assert_eq!(abi.len_offset(), 8);
        assert_eq!(abi.len_size(), 8);
        assert_eq!(abi.total_size(), 16);
        assert_eq!(abi.align(), 8);
    }

    #[test]
    fn shims_match_total_size() {
        let p = plan(8);
        assert_eq!(p.slice_descriptor_size(), p.slice_descriptor().total_size());
        assert_eq!(p.string_descriptor_size(), p.text_descriptor().total_size());
        assert_eq!(p.slice_descriptor_size(), p.string_descriptor_size());
    }

    #[test]
    fn kinds_share_layout_differ_by_tag() {
        let p = plan(8);
        let slice = p.slice_descriptor();
        let text = p.text_descriptor();
        assert_eq!(slice.total_size(), text.total_size());
        assert_eq!(slice.len_offset(), text.len_offset());
        assert_eq!(slice.kind(), FatDescriptorKind::Slice);
        assert_eq!(text.kind(), FatDescriptorKind::TextWindow);
        assert_ne!(slice.kind(), text.kind());
    }

    #[test]
    fn dynamic_trait_descriptor_names_both_pointer_words() {
        let p = plan(8);
        let dynamic = p.dynamic_trait_descriptor();
        assert_eq!(dynamic.instance_offset(), 0);
        assert_eq!(dynamic.table_offset(), 8);
        assert_eq!(dynamic.word_size(), 8);
        assert_eq!(dynamic.total_size(), 16);
        assert_eq!(dynamic.align(), 8);
        assert_eq!(dynamic.total_size(), p.slice_descriptor().total_size());
    }

    #[test]
    fn subslice_offsets_pointer_and_shortens_length() {
        let abi = plan(8).slice_descriptor();
        // base at offset 16, 4-byte elements, [1, 3)
        let resolved = abi.subslice(16, 4, 1, 3).expect("valid subslice");
        assert_eq!(resolved.ptr_delta, 16 + 4);
        assert_eq!(resolved.len, 2);
    }

    #[test]
    fn subslice_full_range_is_identity_length() {
        let abi = plan(8).slice_descriptor();
        let resolved = abi.subslice(0, 4, 0, 4).expect("valid subslice");
        assert_eq!(resolved.ptr_delta, 0);
        assert_eq!(resolved.len, 4);
    }

    #[test]
    fn subslice_rejects_inverted_bounds() {
        let abi = plan(8).slice_descriptor();
        assert_eq!(abi.subslice(0, 4, 3, 1), None);
    }
}

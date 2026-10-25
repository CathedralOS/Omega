//! The format-neutral final image: object plan in, patched bytes and replay
//! evidence out. No container is emitted here - ELF, PE and Mach-O each build
//! their own on top of what this produces.
//!
//! `build_final_image` copies an `ObjectPlan` and a `RelocationPlan` into a
//! `FinalImage` (bytes, symbols, imports, relocations, executable regions).
//! The emitters then hand back a `FinalImageLayout` naming the addresses they
//! chose, and `apply_relocations` patches the fields in place against it.
//!
//! The load-bearing trick is that symbol handles are not remapped. A
//! `FinalImageSymbolHandle` reuses its `ObjectSymbolHandle`'s raw coordinate:
//!
//! ```text
//!   final_image_symbol_handle(symbol) = Handle::from_parts(
//!       symbol.arena_index(), symbol.generation())
//! ```
//!
//! Two distinct `Handle<T>` types with identical eight-byte layout, kept apart
//! by the type parameter alone, and the index is carried across rather than
//! translated. That is sound only because `copy_object_symbols` inserts into an
//! arena the builder just created empty, in one `insert_many` over the object's
//! own iteration order, so entry N lands at index N with generation 1. An
//! object symbol that had ever been removed and reinserted would carry
//! generation 2 or more and the copied handle would resolve to the dummy.
//! `copy_object_relocations` is the only place that re-checks: it runs the
//! copied handle through `symbols.is_valid` and downgrades a handle that does
//! not resolve to `Handle::invalid()` rather than storing a coordinate that
//! points at the wrong symbol.
//!
//! The relocation arithmetic, with the constants the ISA forces on us, lives
//! in `relocations.rs`, which states each kind's field geometry once.
//! `relocation_envelope.rs` turns that same declaration around, as
//! little-endian byte masks, to prove afterwards that only the relocation
//! fields moved: `Aarch64Page21 -> [0xe0, 0xff, 0xff, 0x60]` is exactly
//! `(0b11 << 29) | (0x7ffff << 5)` written out as `to_le_bytes`. The mask
//! table that used to live in the envelope as a hand-kept second copy is gone;
//! `each_relocation_kind_masks_exactly_its_declared_field_bits` pins the single
//! declaration against independent byte masks.
//!
//! Text and data envelopes are deliberately asymmetric. A final `.text` buffer
//! may be LONGER than the encoded input, and only the encoded-length prefix is
//! validated, because the PE and Mach-O emitters append import thunks after the
//! compiler's own code. Initialized data gets no such latitude: every
//! `Data`-section relocation must be `Absolute64`, exactly eight bytes wide,
//! eight-byte aligned, non-overlapping and in bounds.

//! Every evidence structure here is hashed twice, and the compact hash is never
//! allowed to stand for the strong one. The strong lane is SHA-256 over a
//! NUL-terminated versioned domain string; the compact lane is FNV-1a 64. The
//! reason both exist is a report that wants a short number to print, and the
//! reason the short number cannot be trusted is that it collides: the test
//! `compact_collision_cannot_substitute_strong_compiler_text_evidence` builds
//! two evidence values that agree on the fingerprint and differ on the digest,
//! and requires the pair to be rejected.
//!
//! That distinction is enforced by a NAMING RULE rather than by the type
//! system. A compact value must be spelled `*_report_fingerprint` and never a
//! bare `*_fingerprint`, and what checks it is
//! `tests/architecture/native_image_identity.rs`, which reads this crate's
//! source as text and asserts that `pub byte_fingerprint: u64`,
//! `pub text_fingerprint: u64` and `pub inventory_fingerprint: u64` do not
//! appear. A `ReportFingerprint(u64)` newtype would have made it a type error
//! instead. It was not taken, and it is worth being honest that a string search
//! in another crate is the weaker instrument - it cannot see a field introduced
//! through a type alias or a macro.
//!
//! Rebuilding the symbol mapping by name into a hash map was the alternative to
//! reusing handle coordinates. It loses twice: duplicate symbol names have no
//! single answer, and the repository's baseline is arenas over hash maps.
//!
//! When an import carries both a normalized foreign locator and a legacy
//! `import_library` string, the locator wins and the symbol's own spelling is
//! ignored rather than merged. `final_image_keeps_normalized_import_atomic_and
//! _ignores_symbol_spelling` in `tests.rs` pins that by setting the string to
//! `"must-not-win.dll"`.

//! Consumed by `image-elf`, `image-pe`, `image-macho` and
//! `image-emission`, which each take `FinalImage` plus
//! `place_executable_regions` and add their own container.
//!
//! @Cleanup: FNV-1a is open-coded three times. The offset basis
//! `0xcbf2_9ce4_8422_2325` appears in `output.rs`, `relocation_envelope.rs`,
//! `model/executable_regions.rs`, and three of
//! those define their own private `fingerprint_bytes`. Three copies of a hash
//! function is four chances for one of them to drift.
//!
//! @Note: the inventory replay is NOT inert:
//! `validate_placed_executable_region_inventory` runs in production
//! from image-emission's `validate_terminal_image_with_import_count` and the
//! retained-image evidence consumers,
//! and the PE and Mach-O emitters attach thunk footprints through
//! `validate_import_thunk_footprints`, which placement then carries into
//! `PlacedExecutableRegion.footprint`. Only the compiler-entry footprint
//! binding still waits for a caller.
//!
//! @Note: do not decide what is dead here by grepping for type names. The
//! `CompilerEntryRegionBindingEvidence` and `CompilerEntryFootprintBindingEvidence`
//! types have no external occurrence of their names at all, and are both live:
//! `NativeArtifact::recomputed_identity` in `native-artifact/src/native_artifact.rs`
//! reaches them through
//! `output.compiler_entry_region_binding` and destructures the fields without
//! ever spelling the type. Acting on a name grep there would have broken a
//! consumer crate's build.

/// The `[u8; 32]` strong-digest newtype every image evidence and region
/// inventory declares — one wrapper shape the whole crate shares.
macro_rules! image_digest {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            /// Reconstruct the strong digest carried by checked custody
            /// evidence so an independent receiver can recompute the same
            /// commitment without the producing image.
            pub const fn from_digest(digest: [u8; 32]) -> Self {
                Self(digest)
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

/// The rejection carrier every container backend returns when a route refuses
/// while retaining its complete original inputs — one owner for the accessor
/// pair, the `Display` over the carried diagnostic, and the `Error` marker.
///
/// The backend keeps its own carrier declaration: the name, visibility,
/// `#[must_use]` message and retained fields are that route's custody decision
/// and stay written at the route. Invoke with the fields in `into_parts`
/// order; the diagnostic is appended to the returned tuple.
#[macro_export]
macro_rules! rejection_carrier {
    ($name:ident; $($field:ident: $ty:ty),+ $(,)?) => {
        impl $name {
            pub const fn diagnostic(&self) -> &diagnostics::Diagnostic {
                &self.diagnostic
            }

            pub fn into_parts(self) -> ($($ty,)+ diagnostics::Diagnostic) {
                ($(self.$field,)+ self.diagnostic)
            }
        }

        diagnostics::diagnostic_display!($name);

        impl std::error::Error for $name {}
    };
}

mod builder;
pub mod codec_bytes;
mod final_image;
pub(crate) use optimization_core::digest_encoder;
mod output;
mod patch_bytes;
mod relocation_envelope;
mod relocations;
mod symbols;
#[cfg(test)]
mod tests;

pub use builder::{FinalImageInput, build_final_image};
pub use final_image::{
    FinalDataRegion, FinalDataRegionOrigin, FinalExecutableRegion, FinalExecutableRegionOrigin,
    FinalExecutableTextDigest, FinalImage, FinalImageImport, FinalImageImportPlan,
    FinalImageLayout, FinalImageMemory, FinalImageRelocation, FinalImageRelocationTable,
    FinalImageSection, FinalImageSymbol, FinalImageSymbolDigest, FinalImageSymbolHandle,
    FinalInitializedDataDigest, PlacedDataGapBytesDigest, PlacedDataRegionBytesDigest,
    PlacedDataRegionInventory, PlacedDataRegionInventoryDigest, PlacedExecutableGapBytesDigest,
    PlacedExecutableRegion, PlacedExecutableRegionBytesDigest, PlacedExecutableRegionInventory,
    PlacedExecutableRegionInventoryDigest, StateFootprintEvidenceDigest, final_image_symbol_digest,
    place_data_extent, place_data_regions, place_executable_regions,
    validate_placed_data_region_inventory, validate_placed_executable_region_inventory,
};
pub use output::{
    CompilerEntryFootprintBindingDigest, CompilerEntryRegionBindingDigest,
    CompilerFunctionValidationDigest, CompilerFunctionValidationEvidence,
    CompilerTextDerivationDigest, CompilerTextRelocationEnvelopeDigest,
    CompilerTextValidationEvidence, EmittedImageOutput, EncodedCompilerTextDigest,
    ExecutableImageOutput, FinalCompilerTextDigest, ImageOutputKind,
    emitted_direct_executable_output,
};
pub use relocation_envelope::validate_final_text_relocation_envelope;
pub use relocations::apply_relocations;
pub use symbols::{
    final_image_entry_symbol, final_image_imports_symbol, final_image_symbol_address,
    final_image_symbol_name,
};

/// Round `value` up to a multiple of `alignment` (a zero alignment is one).
pub fn align_to(value: usize, alignment: usize) -> usize {
    let alignment = alignment.max(1);
    value.div_ceil(alignment) * alignment
}

/// The `u64` spelling of [`align_to`].
pub fn align_to_u64(value: u64, alignment: u64) -> u64 {
    let alignment = alignment.max(1);
    value.div_ceil(alignment) * alignment
}

//! The format-neutral final image: object plan in, patched bytes and replay
//! evidence out. No container is emitted here - ELF, PE and Mach-O each build
//! their own on top of what this produces.
//!
//! `build_final_image` copies an `ObjectPlan` and a `RelocationPlan` into a
//! `FinalImage` (bytes, symbols, imports, relocations, executable regions).
//! The emitters then hand back a `FinalImageLayout` naming the addresses they
//! chose, and `apply_*_relocations` patches the fields in place against it.
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
//! The relocation arithmetic, with the constants the ISA forces on us:
//!
//! - AArch64 ADRP: pages are 4096 bytes (`address & !0xfff`), the page delta
//!   must lie in `[-2^20, 2^20)`, and the 21-bit immediate splits into
//!   `immlo` at instruction bits 29-30 and `immhi` at bits 5-23, cleared with
//!   `(0b11 << 29) | (0x7ffff << 5)`.
//! - AArch64 ADD page offset: `symbol_address & 0xfff`, shifted into bits
//!   10-21.
//! - x86-64 rel32: `relocation_address = section_address + offset + 4`. That
//!   `+ 4` is an assumption the code never states - that the four-byte
//!   displacement is the last field of the instruction - and it is why the
//!   delta is computed in `i128` before being narrowed to `i32`.
//!
//! `relocation_envelope.rs` carries the same masks a second time, as
//! little-endian byte arrays, to prove afterwards that only the relocation
//! fields moved: `Aarch64Page21 -> [0xe0, 0xff, 0xff, 0x60]` is exactly
//! `(0b11 << 29) | (0x7ffff << 5)` written out. The two tables live in
//! different files and are kept in step by hand.
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

//! Consumed by `omega-image-elf`, `omega-image-pe`, `omega-image-macho` and
//! `omega-image-emission`, which each take `FinalImage` plus
//! `place_executable_regions` and add their own container.
//!
//! @Cleanup: FNV-1a is open-coded four times. The offset basis
//! `0xcbf2_9ce4_8422_2325` appears in `output.rs`, `relocation_envelope.rs`,
//! `model/executable_regions.rs` and `footprint_certificate.rs`, and three of
//! those define their own private `fingerprint_bytes`. Four copies of a hash
//! function is four chances for one of them to drift.
//!
//! @Incomplete: the footprint-certificate lane has no production caller.
//! `FinalFootprintCertificate::current`, `bind_compiler_entry_footprint` and
//! `validate_placed_executable_region_inventory` are reached only from
//! `#[cfg(test)]`, so `PlacedExecutableRegion.footprint` is permanently `None`
//! in every shipped compilation. Their consumer was deleted with the legacy
//! StateGraph route in `f6b3e65350` (2026-08-28). Note before reaching for the
//! delete key: `tests/architecture/native_image_identity.rs:103` reads
//! `footprint_certificate.rs` as source TEXT and asserts literal strings are
//! present, so removing the lane breaks a test that does not break the build.
//!
//! @Note: do not decide what is dead here by grepping for type names. The
//! `CompilerEntryRegionBindingEvidence` and `CompilerEntryFootprintBindingEvidence`
//! types have no external occurrence of their names at all, and are both live:
//! `omega-native-artifact/src/lib.rs:780-796` reaches them through
//! `output.compiler_entry_region_binding` and destructures the fields without
//! ever spelling the type. Acting on a name grep there would have broken a
//! consumer crate's build.

mod aarch64_relocations;
mod builder;
mod footprint_certificate;
mod function_linkage;
mod model;
mod output;
mod patch_bytes;
mod relocation_envelope;
mod symbols;
#[cfg(test)]
mod tests;
mod x86_64_relocations;

pub use aarch64_relocations::apply_aarch64_relocations;
pub use builder::{FinalImageInput, build_final_image};
pub use footprint_certificate::{
    FINAL_FOOTPRINT_CERTIFICATE_MARKER, FinalFootprintCertificate, FinalFootprintCertificateDigest,
    FinalFootprintClass, FinalFootprintCoverage, FinalFootprintCoverageDigest,
    FinalFootprintPlacementBindingDigest,
};
pub use function_linkage::validate_final_image_function_linkage;
pub use model::{
    FinalExecutableRegion, FinalExecutableRegionOrigin, FinalExecutableTextDigest, FinalImage,
    FinalImageImport, FinalImageImportPlan, FinalImageLayout, FinalImageMemory,
    FinalImageRelocation, FinalImageRelocationTable, FinalImageSection, FinalImageSymbol,
    FinalImageSymbolDigest, FinalImageSymbolHandle, FinalImageSymbolTable, PlacedExecutableGap,
    PlacedExecutableGapBytesDigest, PlacedExecutableRegion, PlacedExecutableRegionBytesDigest,
    PlacedExecutableRegionInventory, PlacedExecutableRegionInventoryDigest,
    StateFootprintEvidenceDigest, bind_compiler_entry_footprint, final_image_symbol_digest,
    place_executable_regions, validate_placed_executable_region_inventory,
};
pub use output::{
    CompilerEntryFootprintBindingDigest, CompilerEntryFootprintBindingEvidence,
    CompilerEntryRegionBindingDigest, CompilerEntryRegionBindingEvidence,
    CompilerFunctionValidationDigest, CompilerFunctionValidationEvidence,
    CompilerTextDerivationDigest, CompilerTextRelocationEnvelopeDigest,
    CompilerTextValidationEvidence, EmittedImageOutput, EncodedCompilerTextDigest,
    ExecutableImageOutput, FinalCompilerTextDigest, ImageOutputKind,
    emitted_direct_executable_output,
};
pub use relocation_envelope::validate_final_text_relocation_envelope;
pub use symbols::{
    final_image_imports_symbol, final_image_symbol_address, final_image_symbol_name,
};
pub use x86_64_relocations::apply_x86_64_relocations;

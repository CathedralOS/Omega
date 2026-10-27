//! Two object-file lanes that share a crate and almost nothing else, plus the
//! symbol-naming rules both obey.
//!
//! Start at `object_plan.rs` for the mutable lane; `relocation_free_object`
//! is the immutable lane with its publication and text-section children,
//! `fragment_container` is its optimizer entrance, and `names` and
//! `target_matrix` hold the spellings and the target policy both lanes obey.
//!
//! The first lane is mutable and arena-backed: `ObjectPlan` and
//! `RelocationPlan`, which the image builders consume and which serializes as
//! the OMGOBJ container. The second is immutable and SHA-256-identified:
//! `RelocationFreeObjectPlan`, serializing as OMGTRO, which is what the
//! optimization pipeline consumes. Its input `RelocationFreeTextSectionPlacement`
//! is machine-code representation data, re-exported here for existing consumers.
//! They are not two views of one thing and must not be merged.
//!
//! Both headers are 44 bytes, which is a coincidence and not a shared design:
//!
//! ```text
//!   OMGOBJ   0  magic b"OMGOBJ\0\0"      (six characters, two NUL pad)
//!            8  u32 version = 6
//!           12  u32 architecture, u32 object format
//!           20  u64 text length, u64 data length, u64 bss length
//!           44  symbols, relocations, then raw text and data verbatim
//!
//!   OMGTRO   0  magic b"OMGTRO\0\0"
//!            8  u32 version = 2
//!           12  32-byte RelocationFreeObjectPlanIdentity
//!           44  body
//! ```
//!
//! Below the header the two formats disagree on widths, deliberately. OMGOBJ
//! length-prefixes strings with a `u32` and writes every enum tag as a `u32`;
//! OMGTRO length-prefixes with a `u64` and writes every tag as a single byte.
//! Neither writes NUL terminators anywhere. Everything is little-endian.
//!
//! Tag tables are 1-based so that a zero byte is never a valid discriminant -
//! architecture, object format, symbol kind, section kind, relocation kind,
//! relocation origin. The separate machine-code text-section identity's
//! `MachineAlternativeFamily` table is zero-based, running
//! `CompareI64Zero = 0` through `CallScalar = 13`. A zero byte is a legal family
//! tag there and an invalid tag in these object-format tables.
//!
//! `validate_relocation_free_object` admits five of the six architecture and
//! object-format combinations - Aarch64 with Elf or MachO, X86_64 with Elf,
//! Coff or MachO - each additionally requiring `pointer_size` and
//! `pointer_alignment` of exactly 8. Aarch64 with Coff rejects as
//! `NonCanonicalTarget`. Those five pairs are the declared object target
//! matrix in `target_matrix`: every target-derived spelling (entry symbol,
//! section names) and the canonical text alignment resolve through
//! `object_target_policy`, and an undeclared pair answers `None` there rather
//! than matching a format wildcard.

//! The architecture and object-format tag mappings are written out twice:
//! as `u8` in the relocation-free object codec and in the machine-code
//! representation's text-section identity. A shared
//! `to_tag()` on the enums is the obvious
//! cleanup and would be wrong. These are independently versioned wire
//! formats, so one
//! shared table means a change made for one format silently changes the
//! other, and silently reinterprets every text-section identity already hashed and
//! stored. The duplication is the version boundary.
//!
//! `RelocationOrigin::SemanticOperation` and `RelocationOrigin::SemanticEdge`
//! carry byte-identical payloads - a symbol handle and a `u64` - and still get
//! separate variants and separate tags. One variant with a namespace field was
//! rejected because operation identities and edge identities are disjoint
//! namespaces whose raw integers legitimately collide, so a shared variant lets
//! an edge identity be read as an operation identity with no type error. The
//! test builds both from the integer 7 to make exactly that point.
//!
//! `object_symbol_handle_by_foreign_locator` and `object_function_symbol` scan
//! the whole remaining iterator after their first match and fail closed if a
//! second one exists, rather than returning the first hit. That is O(n) per
//! lookup against a hash map's O(1), and it buys the property that an ambiguous
//! join is a hard failure instead of a coin flip. The failure is spelled ZII:
//! the answer is `Handle::invalid()`, not `Option::None`, so a caller that
//! forgets to check resolves to the dummy entry rather than unwrapping.
//!
//! Five single-variant enums are still written to the wire as an explicit tag
//! byte the decoder compares against a literal 1. Omitting a field that has one
//! possible value is the tempting simplification. The byte reserves the slot: a
//! second policy can be added without a container version bump, and an old
//! encoder's output is rejected by tag rather than misparsed at whatever field
//! follows.
//!
//! Function symbol names are derived from the compiler-private
//! `MachineFunctionIdentity` - arena index AND generation for both machine and
//! state, plus the segment index - and never from the source spelling.
//! Independently chosen source names may coincide; compiler-private identity
//! cannot. Carrying the generation as well as the index is what stops a freed
//! and reused arena slot from colliding with the symbol it replaced.

//! Consumed by `image` (which copies an `ObjectPlan` and a
//! `RelocationPlan` into its final image) and by the optimization pipeline
//! through the relocation-free lane.
//!
//! @Note: OMGTRO version 2 carries a trace beside the constant:
//! it is the version that added the normalized-foreign import plan
//! (`normalized_imports` plus `unresolved_normalized_foreign_calls`), and the
//! test `omgtro_version_2_covers_normalized_foreign_imports` pins it.

pub(crate) use optimization_core::codec_bytes;
/// The foundation's bounded byte cursor, re-exported as this crate's decoder
/// primitive: it already carries every read the wire codecs perform, and
/// `From<Truncated>` reaches each format's own error enum.
pub(crate) use optimization_core::cursor;
mod fragment_container;
mod names;
mod object_plan;
mod relocation_free_object;
mod target_matrix;

pub use fragment_container::{
    RelocationFreeObjectContainerError, StagedOptimizedRelocationFreeObjectContainer,
    stage_optimized_relocation_free_object_container,
    validate_optimized_relocation_free_object_container,
};
/// The relocation-free lane's text-section input is machine-code
/// representation data, re-exported here for existing consumers.
pub use machine_code::{
    InternalMachineCallResolutionKind, InternalMachineCallResolutionState,
    NormalizedForeignCallResolutionKind, NormalizedForeignCallResolutionState, PlacedBlockSpan,
    PlacedFunctionFragment, PlacedInstructionSpan, PlacedInternalMachineCallResolution,
    PlacedNormalizedForeignCallResolution, RelocationFreeTextSectionPlacement,
    TextSectionPlacementPolicy, TextSectionRelocationRequirements,
    relocation_free_text_section_identity,
};
pub use names::{
    entry_symbol_name, normalized_foreign_import_symbol_name, normalized_import_symbol_name,
    object_function_reference, object_function_symbol, object_symbol_handle_by_foreign_locator,
    object_symbol_handle_by_import_locator, object_symbol_name, section_name,
};
pub use object_plan::ObjectPlan;
pub use object_plan::container::ObjectContainerOutput;
pub use object_plan::relocations::{
    RelocationKind, RelocationOrigin, RelocationPlan, RelocationRecord,
    invocation_relocation_origin, relocation_kind_tag,
};
pub use object_plan::sections::{SectionKind, SectionPlan};
pub use object_plan::symbols::{
    FunctionSymbolPlan, NormalizedImportPlan, ObjectSymbolHandle, SymbolKind, SymbolPlan,
    SymbolSection,
};
pub use relocation_free_object::publication;
pub(crate) use relocation_free_object::publication::{
    FunctionFragmentObjectContainerManifest, FunctionFragmentObjectContainerStage,
    FunctionFragmentObjectContainerUnavailableData, relocation_free_object_statistics,
};
pub use relocation_free_object::text_section;
pub(crate) use relocation_free_object::text_section::{
    RelocationFreeObjectFromTextError, construct_relocation_free_object_from_text,
    validate_relocation_free_object_from_text,
};
pub use relocation_free_object::{
    ObjectLocalSymbolId, RelocationFreeFunctionSymbol, RelocationFreeObjectNormalizedImport,
    RelocationFreeObjectPlan, RelocationFreeObjectRelocationRequirements,
    RelocationFreeObjectSymbolLinkage, RelocationFreeObjectSymbolPolicy,
    RelocationFreeObjectSymbolRole, RelocationFreeObjectTextSection,
    RelocationFreeObjectUnresolvedForeignCall, canonical_normalized_foreign_import_symbol_name,
    canonical_private_machine_symbol_name,
};
pub(crate) use relocation_free_object::{
    RelocationFreeObjectContainer, RelocationFreeObjectDecodeError, RelocationFreeObjectError,
    decode_relocation_free_object, encode_relocation_free_object, validate_relocation_free_object,
};
pub(crate) use target_matrix::object_target_policy;

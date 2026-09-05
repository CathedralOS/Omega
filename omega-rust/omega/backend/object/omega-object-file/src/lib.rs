//! Two object-file lanes that share a crate and almost nothing else, plus the
//! symbol-naming rules both obey.
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
//!            8  u32 version = 1
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
//! `CompareI64Zero = 0` through `CallI64 = 13`. A zero byte is a legal family
//! tag there and an invalid tag in these object-format tables.
//!
//! `validate_relocation_free_object` admits four of the six architecture and
//! object-format combinations - Aarch64 with Elf or MachO, X86_64 with Elf or
//! Coff - each additionally requiring `pointer_size` and `pointer_alignment` of
//! exactly 8. Aarch64 with Coff and X86_64 with MachO reject as
//! `NonCanonicalTarget`.

//! The architecture and object-format tag mappings are written out three times:
//! as `u32` in `container/ids.rs`, as `u8` in the relocation-free object codec,
//! and in the machine-code representation's text-section identity. A shared
//! `to_tag()` on the enums is the obvious
//! cleanup and would be wrong. These are three independently versioned wire
//! formats - OMGOBJ v6, OMGTRO v1, and the v3 text-section hash schema - so one
//! shared table means a change made for one format silently changes the other
//! two, and silently reinterprets every text-section identity already hashed and
//! stored. The duplication is the version boundary. Worth knowing before you
//! trust it: only the `container/ids.rs` copy is pinned by a test.
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

//! Consumed by `omega-image` (which copies an `ObjectPlan` and a
//! `RelocationPlan` into its final image) and by the optimization pipeline
//! through the relocation-free lane.
//!
//! @Note: `OBJECT_CONTAINER_VERSION` is 6 and the constant says nothing about
//! why. The only surviving record is the name of the test beside it,
//! `object_container_version_covers_semantic_edge_relocation_origins`: version 6
//! is the one that added the `SemanticEdge` origin, so an older reader cannot
//! mistake tag 4 for something it knows. Do not bump it without leaving the same
//! kind of trace.
//!
//! @Cleanup: `storage_region_symbol_name` in `names.rs` has no caller anywhere
//! in the workspace, its own tests included. It is the only reader of
//! `omega_core::runtime_storage::RuntimeStorageRegion`, so removing it also
//! strands that module - see the header of `omega-core`, which wrongly called
//! that module live on the strength of this import until the call chain was
//! actually followed.

mod container;
mod names;
mod plan;
mod relocation_free_object;
mod relocation_free_text_section;
mod relocations;
mod sections;
mod symbols;

pub use container::*;
pub use names::*;
pub use plan::*;
pub use relocation_free_object::*;
pub use relocation_free_text_section::*;
pub use relocations::*;
pub use sections::*;
pub use symbols::*;

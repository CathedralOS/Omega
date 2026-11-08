/*
We provide two distinct object-file lanes and the target/symbol-naming rules
both obey. The mutable arena-backed lane (`object_plan`) defines `ObjectPlan`
and `RelocationPlan`, which image builders consume and which serialize as the
`OMGOBJ` container. The immutable SHA-256-identified lane
(`relocation_free_object` and `fragment_container`) defines
`RelocationFreeObjectPlan`, which the optimization pipeline consumes and which
serializes as the `OMGTRO` container, alongside its `OMGTOM` publication
manifest. Both lanes resolve target-derived entry and section spellings through
the closed five-pair target policy in `target_matrix` and compiler-private
symbol names in `names`.
*/

/*
Although both `OMGOBJ` and `OMGTRO` begin with a 44-byte header, they serve
different pipeline contracts and intentionally diverge in wire widths (`u32`
versus `u64` length prefixes, 4-byte versus 1-byte enum tags) and tag tables so
changes to one format never silently reinterpret hashed identities in the other:

```text
  OMGOBJ   0  magic b"OMGOBJ\0\0"      (six characters, two NUL pad)
           8  u32 version = 6
          12  u32 architecture, u32 object format
          20  u64 text length, u64 data length, u64 bss length
          44  symbols, relocations, then raw text and data verbatim

  OMGTRO   0  magic b"OMGTRO\0\0"
           8  u32 version = 5
          12  32-byte RelocationFreeObjectPlanIdentity
          44  body
```

We enforce fail-closed semantics throughout both lanes:
- `target_matrix` admits only the five canonical 64-bit `(architecture, object_format)`
  pairs (`Aarch64` with `Elf`/`MachO`, `X86_64` with `Elf`/`Coff`/`MachO`),
  rejecting `(Aarch64, Coff)` rather than falling back to a format wildcard.
- `RelocationOrigin::SemanticOperation` and `RelocationOrigin::SemanticEdge`
  remain distinct variants and wire tags even though both carry a symbol handle
  and a `u64`, because operation and edge identities live in disjoint integer
  namespaces that can collide as raw numbers.
- `object_symbol_handle_by_foreign_locator` and `object_function_symbol` scan
  the entire remaining table after a match and return an invalid handle (or
  `None`) if a duplicate exists, turning ambiguous joins into hard failures.
- Single-variant enums in `OMGTRO` and `OMGTOM` still write an explicit tag byte
  (`1`) on the wire so future policy variants can be added without misparsing
  adjacent fields.
- Function symbol names derive from the compiler-private `MachineFunctionIdentity`
  (arena index, generation, and segment index) rather than source names,
  preventing collisions across reused arena slots or identical source names.
*/

/*
`image` consumes `ObjectPlan` and `RelocationPlan` to assemble final native
executables, while the optimization pipeline stages and validates
`StagedOptimizedRelocationFreeObjectContainer` through `fragment_container`.
We re-export the machine-code text-section placement types here so downstream
consumers can inspect placed fragments and relocation requirements from one
boundary.
*/

pub(crate) use optimization_core::codec_bytes;
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
pub use object_plan::{
    FunctionSymbolPlan, NormalizedImportPlan, ObjectContainerOutput, ObjectPlan,
    ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationPlan, RelocationRecord,
    SectionKind, SectionPlan, SymbolKind, SymbolPlan, SymbolSection, invocation_relocation_origin,
    relocation_kind_tag,
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

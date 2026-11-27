/*
`publish_function_object` turns one placed native body into linkable text and
data, function and import symbols, and object-relative relocations. A
validator request also retains the placed text so the object can be rebuilt
and compared. `body_object` places literals, descriptors and private
references before image composition can bind them.
*/

/*
Image composition reads `ObjectPlan` and `RelocationPlan` directly.

We use the following rules when composing plans.
- `target_matrix` names one row for each of the six 64-bit `(architecture, object_format)`
  pairs (`Aarch64` and `X86_64`, each with `Elf`/`Coff`/`MachO`) in an exhaustive
  match, so a new architecture or format fails to compile instead of falling back
  to a wildcard row.
- `RelocationOrigin::SemanticOperation` and `RelocationOrigin::SemanticEdge`
  remain distinct variants even though both carry a symbol handle
  and a `u64`, because operation and edge identities live in disjoint integer
  namespaces that can collide as raw numbers.
- `object_symbol_handle_by_foreign_locator` and `object_function_symbol` scan
  the entire remaining table after a match and return an invalid handle (or
  `None`) if a duplicate exists, turning ambiguous joins into hard failures.
- Function symbol names never use source names. A placed machine is named
  `omega_terminal_machine_{MachineId}`, or the target's entry symbol for the
  semantic entry. A callback target is named from its `MachineFunctionIdentity`:
  the caller's vocabulary marker and program fingerprint plus the callback-thunk
  placement index (`private_function_symbol_name`). Identical source names
  therefore cannot collide.
*/

/*
`image` consumes `ObjectPlan` and `RelocationPlan` to assemble final native
executables, while the optimization pipeline stages and validates
`StagedOptimizedRelocationFreeObjectContainer` through `fragment_container`.
We re-export the machine-code text-section placement types here so downstream
consumers can inspect placed fragments and relocation requirements from one
boundary.
*/

mod bridge_object;
pub use bridge_object::{
    BridgeCallRelocation, CallableEntryTransport, ForeignAdapterObject, ForeignAdapterObjectParts,
    NativeBridgeObject, NativeEntryBridge, NativeForeignBridge, PrivateEntryObject,
};
pub mod body_object;
pub use body_object::{
    FunctionObjectSections, ObjectByteLiteralAddress, ObjectByteLiteralPool, ObjectFunctionLocation,
};
mod foreign_calls;
pub use foreign_calls::{
    ForeignCallPlacementError, ObjectForeignCall, derive_normalized_foreign_call_custody,
    fragment_instruction_span, place_foreign_call, resolve_instruction_sequence,
};
mod fragment_container;
mod names;
mod object_plan;
mod relocation_free_object;
mod target_matrix;

pub use fragment_container::{
    ObjectPublication, PublishedFunctionObject, RelocationFreeObjectContainerError,
    StagedOptimizedRelocationFreeObjectContainer, publish_function_object,
    validate_optimized_relocation_free_object_container,
};
pub use machine_code::{
    InternalMachineCallResolutionKind, InternalMachineCallResolutionState,
    NormalizedForeignCallResolutionKind, NormalizedForeignCallResolutionState, PlacedBlockSpan,
    PlacedFunctionFragment, PlacedInstructionSpan, PlacedInternalMachineCallResolution,
    PlacedNormalizedForeignCallResolution, RelocationFreeTextSectionPlacement,
    TextSectionPlacementPolicy, TextSectionRelocationRequirements,
};
pub use names::{
    entry_symbol_name, normalized_foreign_import_symbol_name, normalized_import_symbol_name,
    object_function_reference, object_function_symbol, object_symbol_handle_by_foreign_locator,
    object_symbol_handle_by_import_locator, object_symbol_name, private_function_symbol_name,
    section_name,
};
pub use object_plan::{
    FunctionSymbolPlan, FunctionUnwindPlan, ImportRelocationError, NormalizedImportPlan,
    ObjectContainerOutput, ObjectPlan, ObjectSymbolHandle, PlacedImportCall, RelocationKind,
    RelocationOrigin, RelocationPlan, RelocationRecord, SectionKind, SectionPlan, SymbolKind,
    SymbolPlan, SymbolSection, invocation_relocation_origin, relocation_kind_tag,
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
    RelocationFreeObjectError, validate_relocation_free_object,
};
pub(crate) use target_matrix::object_target_policy;

pub mod program_entry_object;

use super::{
    CompilerBodyPlaceCopyShape, CompilerBodyPlaceIntegerWriteShape, OutboundCallRelocationTarget,
    aarch64_outbound_syscall_operand, compiler_body_place_copy_shape,
    compiler_body_place_integer_write_shape, compiler_instruction_non_relocation_bits_match,
    compiler_place_binary_write_address_sites, compiler_place_convert_write_address_sites,
    compiler_place_copy_address_sites, compiler_place_integer_write_address_sites,
    compiler_place_value_address_sites, compiler_runtime_value_compare_address_sites,
    emit_checked_executable_image, encode_aarch64_indirect_call_replay,
    outbound_syscall_argument_data_sites, outbound_syscall_argument_storage_sites,
    require_compiler_instruction_footprint, validate_checked_instruction_bytes,
    validate_compiler_data_address_relocations, validate_compiler_function_instruction_boundaries,
    validate_compiler_runtime_text_relocations, validate_executable_region_enumeration,
    validate_final_text_relocation_envelope,
};
use crate::ExecutableImageInput;
use omega_image::PlacedExecutableRegionInventory;
use omega_object_file::{
    ObjectPlan, RelocationKind, RelocationOrigin, RelocationPlan, RelocationRecord, SectionKind,
    SymbolKind, SymbolPlan, SymbolSection,
};
use omega_target::NativeTarget;
use psi_arena::Handle;

mod final_validation;
mod guard_assembly;
mod place_replay;

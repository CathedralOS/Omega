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
    FINAL_FOOTPRINT_CERTIFICATE_MARKER, FinalFootprintCertificate, FinalFootprintClass,
    FinalFootprintCoverage,
};
pub use function_linkage::validate_final_image_function_linkage;
pub use model::{
    FinalExecutableRegion, FinalExecutableRegionOrigin, FinalImage, FinalImageImport,
    FinalImageImportPlan, FinalImageLayout, FinalImageMemory, FinalImageRelocation,
    FinalImageRelocationTable, FinalImageSection, FinalImageSymbol, FinalImageSymbolHandle,
    FinalImageSymbolTable, PlacedExecutableGap, PlacedExecutableRegion,
    PlacedExecutableRegionInventory, bind_compiler_entry_footprint, place_executable_regions,
    validate_placed_executable_region_inventory,
};
pub use output::{
    CompilerEntryFootprintBindingEvidence, CompilerEntryRegionBindingEvidence,
    CompilerFunctionValidationEvidence, CompilerTextValidationEvidence, EmittedImageOutput,
    ExecutableImageOutput, ImageOutputKind, emitted_direct_executable_output,
};
pub use relocation_envelope::validate_final_text_relocation_envelope;
pub use symbols::{
    final_image_imports_symbol, final_image_symbol_address, final_image_symbol_name,
};
pub use x86_64_relocations::apply_x86_64_relocations;

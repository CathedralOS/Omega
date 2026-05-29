mod aarch64_relocations;
mod builder;
mod model;
mod output;
mod symbols;
mod x86_64_relocations;

pub use aarch64_relocations::apply_aarch64_relocations;
pub use builder::{FinalImageInput, build_final_image};
pub use model::{
    FinalImage, FinalImageImport, FinalImageLayout, FinalImageRelocation, FinalImageSection,
    FinalImageSymbol, FinalImageSymbolHandle,
};
pub use output::{
    EmittedImageOutput, ExecutableImageOutput, ImageOutputKind, emitted_direct_executable_output,
};
pub use symbols::{
    final_image_imports_symbol, final_image_symbol_address, final_image_symbol_name,
};
pub use x86_64_relocations::apply_x86_64_relocations;

use crate::plan::NativePlan;
pub use omega_image::{
    FinalImage, FinalImageImport, FinalImageInput, FinalImageLayout, FinalImageRelocation,
    FinalImageSection, FinalImageSymbol, FinalImageSymbolHandle, apply_aarch64_relocations,
    build_final_image as build_image,
};

pub fn build_final_image(native_plan: &NativePlan) -> FinalImage {
    build_image(FinalImageInput {
        target: native_plan.target,
        object: &native_plan.object,
        relocations: &native_plan.relocations,
        text_bytes: native_plan.machine_code.bytes.storage_slice(),
        data_bytes: native_plan.data.bytes.storage_slice(),
    })
}

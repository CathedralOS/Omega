/*
We define the emitter output carriers: `ExecutableImageOutput` from a container
writer and `EmittedImageOutput`, which also carries the final-image counts that
native artifacts bind into their identity.
*/

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableImageOutput {
    pub bytes: Vec<u8>,
    pub final_image_layout: crate::FinalImageLayout,
    pub final_text_bytes: Vec<u8>,
    pub final_data_bytes: Vec<u8>,
    pub final_import_data_bytes: Vec<u8>,
    pub file_name: String,
    pub format: String,
    pub text_bytes: usize,
    pub data_bytes: usize,
    pub bss_bytes: usize,
    pub symbols: usize,
    pub imports: usize,
    pub relocations: usize,
    pub executable_regions: crate::PlacedExecutableRegionInventory,
    pub data_regions: crate::PlacedDataRegionInventory,
    pub import_data_regions: crate::PlacedDataRegionInventory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedImageOutput {
    pub bytes: Vec<u8>,
    pub final_image_layout: crate::FinalImageLayout,
    pub final_text_bytes: Vec<u8>,
    pub final_data_bytes: Vec<u8>,
    pub final_import_data_bytes: Vec<u8>,
    pub callback_placement_identity_report_fingerprint: u64,
    pub file_name: String,
    pub format: String,
    pub text_bytes: usize,
    pub data_bytes: usize,
    pub bss_bytes: usize,
    pub symbols: usize,
    pub relocations: usize,
    pub final_image_symbols: usize,
    pub final_image_imports: usize,
    pub final_image_relocations: usize,
    pub executable_regions: crate::PlacedExecutableRegionInventory,
    pub data_regions: crate::PlacedDataRegionInventory,
    pub import_data_regions: crate::PlacedDataRegionInventory,
}

impl EmittedImageOutput {
    pub fn validate_final_initialized_data_relocation_envelope(
        &self,
        encoded_data_bytes: &[u8],
        relocations: &object_file::RelocationPlan,
    ) -> Result<(), diagnostics::Diagnostic> {
        crate::relocation_envelope::validate_final_initialized_data_relocation_envelope(
            encoded_data_bytes,
            &self.final_data_bytes,
            relocations,
        )
    }
}

pub fn emitted_direct_executable_output(output: ExecutableImageOutput) -> EmittedImageOutput {
    EmittedImageOutput {
        bytes: output.bytes,
        final_image_layout: output.final_image_layout,
        final_text_bytes: output.final_text_bytes,
        final_data_bytes: output.final_data_bytes,
        final_import_data_bytes: output.final_import_data_bytes,
        callback_placement_identity_report_fingerprint: 0,
        file_name: output.file_name,
        format: output.format,
        text_bytes: output.text_bytes,
        data_bytes: output.data_bytes,
        bss_bytes: output.bss_bytes,
        symbols: output.symbols,
        relocations: output.relocations,
        final_image_symbols: output.symbols,
        final_image_imports: output.imports,
        final_image_relocations: output.relocations,
        executable_regions: output.executable_regions,
        data_regions: output.data_regions,
        import_data_regions: output.import_data_regions,
    }
}

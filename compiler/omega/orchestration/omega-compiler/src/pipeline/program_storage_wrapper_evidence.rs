//! Final-image evidence for the emitted receiver-free program-storage wrapper.
//!
//! This joins the already sealed bridge identities to the checked executable
//! image. It carries no installation geometry, root authority, or claim that
//! the platform invoked the resulting bytes.

use super::{ProgramStorageEntryDiagnostic, ProgramStorageEntryNativeBridgePlan};
use omega_control_flow::MachineFunctionIdentity;
use omega_image::{
    CompilerFunctionValidationEvidence, CompilerTextValidationEvidence, EmittedImageOutput,
    FinalExecutableRegionOrigin,
};
use omega_object_file::{RelocationKind, RelocationOrigin, SectionKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryEmittedWrapperEvidence {
    wrapper_identity: MachineFunctionIdentity,
    continuation_identity: MachineFunctionIdentity,
    wrapper_symbol: String,
    continuation_symbol: String,
    wrapper_section_offset: usize,
    wrapper_address: u64,
    wrapper_byte_count: usize,
    wrapper_byte_fingerprint: u64,
    continuation_section_offset: usize,
    continuation_address: u64,
    continuation_byte_count: usize,
    continuation_byte_fingerprint: u64,
    call_section_offset: usize,
    final_call_bytes: [u8; 5],
    arrival: super::program_storage_wrapper_arrival::ProgramStorageEntryEmittedArrivalEvidence,
    compiler_text_validation: CompilerTextValidationEvidence,
    compiler_function_validation: CompilerFunctionValidationEvidence,
    executable_inventory_fingerprint: u64,
}

impl ProgramStorageEntryEmittedWrapperEvidence {
    pub const fn wrapper_identity(&self) -> MachineFunctionIdentity {
        self.wrapper_identity
    }

    pub const fn continuation_identity(&self) -> MachineFunctionIdentity {
        self.continuation_identity
    }

    pub fn wrapper_symbol(&self) -> &str {
        &self.wrapper_symbol
    }

    pub fn continuation_symbol(&self) -> &str {
        &self.continuation_symbol
    }

    pub const fn wrapper_section_offset(&self) -> usize {
        self.wrapper_section_offset
    }

    pub const fn wrapper_address(&self) -> u64 {
        self.wrapper_address
    }

    pub const fn wrapper_byte_count(&self) -> usize {
        self.wrapper_byte_count
    }

    pub const fn wrapper_byte_fingerprint(&self) -> u64 {
        self.wrapper_byte_fingerprint
    }

    pub const fn continuation_section_offset(&self) -> usize {
        self.continuation_section_offset
    }

    pub const fn continuation_address(&self) -> u64 {
        self.continuation_address
    }

    pub const fn continuation_byte_count(&self) -> usize {
        self.continuation_byte_count
    }

    pub const fn continuation_byte_fingerprint(&self) -> u64 {
        self.continuation_byte_fingerprint
    }

    pub const fn call_section_offset(&self) -> usize {
        self.call_section_offset
    }

    pub const fn final_call_bytes(&self) -> &[u8; 5] {
        &self.final_call_bytes
    }

    /// Exact final wrapper rows that consume the retained physical arrival
    /// placements. This remains compile-time image evidence only.
    pub const fn arrival(
        &self,
    ) -> &super::program_storage_wrapper_arrival::ProgramStorageEntryEmittedArrivalEvidence {
        &self.arrival
    }

    pub const fn compiler_text_validation(&self) -> CompilerTextValidationEvidence {
        self.compiler_text_validation
    }

    pub const fn compiler_function_validation(&self) -> CompilerFunctionValidationEvidence {
        self.compiler_function_validation
    }

    pub const fn executable_inventory_fingerprint(&self) -> u64 {
        self.executable_inventory_fingerprint
    }
}

pub(super) fn bind_final_program_storage_entry_wrapper_evidence(
    bridge: &ProgramStorageEntryNativeBridgePlan,
    backend: &omega_backend_plan::BackendPlan,
    image: &EmittedImageOutput,
) -> Result<ProgramStorageEntryEmittedWrapperEvidence, ProgramStorageEntryDiagnostic> {
    let template = bridge.wrapper_body_template().ok_or_else(|| {
        ProgramStorageEntryDiagnostic(
            "final program-storage wrapper evidence requires the receiver-free emitted template"
                .into(),
        )
    })?;
    if bridge.entry_function_identity() != template.wrapper_identity()
        || template.continuation_identity()
            != MachineFunctionIdentity::source(bridge.continuation_key())
    {
        return Err(ProgramStorageEntryDiagnostic(
            "final program-storage wrapper identities drifted from the sealed bridge".into(),
        ));
    }
    let compiler_text_validation = image.compiler_text_validation.ok_or_else(|| {
        ProgramStorageEntryDiagnostic(
            "final program-storage wrapper image lacks compiler-text validation evidence".into(),
        )
    })?;
    let compiler_function_validation = image.compiler_function_validation.ok_or_else(|| {
        ProgramStorageEntryDiagnostic(
            "final program-storage wrapper image lacks compiler-function validation evidence"
                .into(),
        )
    })?;

    let (wrapper_handle, wrapper_symbol) =
        omega_object_file::object_function_symbol(&backend.object, template.wrapper_identity())
            .ok_or_else(|| {
                ProgramStorageEntryDiagnostic(
                    "final program-storage wrapper lost its exact object linkage".into(),
                )
            })?;
    let (continuation_handle, continuation_symbol) = omega_object_file::object_function_symbol(
        &backend.object,
        template.continuation_identity(),
    )
    .ok_or_else(|| {
        ProgramStorageEntryDiagnostic(
            "final program-storage wrapper lost its exact Source linkage".into(),
        )
    })?;
    if wrapper_handle != backend.object.layout.entry_symbol
        || wrapper_symbol.name != bridge.entry_symbol()
        || wrapper_symbol.offset != bridge.entry_text_offset()
        || wrapper_symbol.size != bridge.entry_text_size()
        || continuation_symbol.name != bridge.continuation_link_symbol()
        || continuation_symbol.offset != bridge.continuation_text_offset()
        || continuation_symbol.size != bridge.continuation_text_size()
    {
        return Err(ProgramStorageEntryDiagnostic(
            "final program-storage wrapper object entry or Source interval drifted".into(),
        ));
    }

    let exact_region = |symbol: &omega_object_file::SymbolPlan| {
        let matches = image
            .executable_regions
            .regions
            .iter()
            .filter(|region| {
                region.origin == FinalExecutableRegionOrigin::CompilerFunction
                    && region.symbol == symbol.name
                    && region.section_offset == symbol.offset
                    && region.byte_count == symbol.size
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [region] => Ok(*region),
            _ => Err(ProgramStorageEntryDiagnostic(format!(
                "final image does not retain one exact compiler-function region for `{}`",
                symbol.name
            ))),
        }
    };
    let wrapper_region = exact_region(wrapper_symbol)?;
    let continuation_region = exact_region(continuation_symbol)?;

    let call_relocations = backend
        .relocations
        .records()
        .filter(|(_, relocation)| {
            matches!(
                relocation.origin,
                RelocationOrigin::Instruction {
                    function_symbol_handle,
                    ..
                } if function_symbol_handle == wrapper_handle
            )
        })
        .map(|(_, relocation)| relocation)
        .collect::<Vec<_>>();
    let [call_relocation] = call_relocations.as_slice() else {
        return Err(ProgramStorageEntryDiagnostic(
            "final program-storage wrapper does not retain one exact Source-call relocation".into(),
        ));
    };
    if call_relocation.section != SectionKind::Text
        || call_relocation.symbol_handle != continuation_handle
        || call_relocation.kind != RelocationKind::X86_64Relative32
        || call_relocation.byte_width != 4
        || call_relocation.addend != 0
        || call_relocation.offset == 0
    {
        return Err(ProgramStorageEntryDiagnostic(
            "final program-storage wrapper Source-call relocation shape drifted".into(),
        ));
    }
    let call_section_offset = call_relocation.offset - 1;
    let expected_call = validate_final_source_call(
        &image.final_text_bytes,
        wrapper_symbol.offset,
        wrapper_symbol.size,
        continuation_symbol.offset,
        call_section_offset,
    )?;
    let arrival = super::program_storage_wrapper_arrival::bind_final_program_storage_entry_wrapper_arrival_evidence(
        bridge,
        backend,
        image,
    )?;

    Ok(ProgramStorageEntryEmittedWrapperEvidence {
        wrapper_identity: template.wrapper_identity(),
        continuation_identity: template.continuation_identity(),
        wrapper_symbol: wrapper_symbol.name.clone(),
        continuation_symbol: continuation_symbol.name.clone(),
        wrapper_section_offset: wrapper_region.section_offset,
        wrapper_address: wrapper_region.address,
        wrapper_byte_count: wrapper_region.byte_count,
        wrapper_byte_fingerprint: wrapper_region.byte_fingerprint,
        continuation_section_offset: continuation_region.section_offset,
        continuation_address: continuation_region.address,
        continuation_byte_count: continuation_region.byte_count,
        continuation_byte_fingerprint: continuation_region.byte_fingerprint,
        call_section_offset,
        final_call_bytes: expected_call,
        arrival,
        compiler_text_validation,
        compiler_function_validation,
        executable_inventory_fingerprint: image.executable_regions.inventory_fingerprint,
    })
}

fn validate_final_source_call(
    final_text_bytes: &[u8],
    wrapper_offset: usize,
    wrapper_size: usize,
    continuation_offset: usize,
    call_offset: usize,
) -> Result<[u8; 5], ProgramStorageEntryDiagnostic> {
    let wrapper_end = wrapper_offset.checked_add(wrapper_size).ok_or_else(|| {
        ProgramStorageEntryDiagnostic("final program-storage wrapper interval overflows".into())
    })?;
    let call_end = call_offset.checked_add(5).ok_or_else(|| {
        ProgramStorageEntryDiagnostic(
            "final program-storage wrapper call interval overflows".into(),
        )
    })?;
    let final_call_slice = final_text_bytes.get(call_offset..call_end).ok_or_else(|| {
        ProgramStorageEntryDiagnostic(
            "final program-storage wrapper call lies outside checked text".into(),
        )
    })?;
    let displacement = i64::try_from(continuation_offset)
        .ok()
        .and_then(|target| i64::try_from(call_end).ok().map(|next| target - next))
        .and_then(|value| i32::try_from(value).ok())
        .ok_or_else(|| {
            ProgramStorageEntryDiagnostic(
                "final program-storage wrapper Source-call displacement is out of rel32 range"
                    .into(),
            )
        })?;
    let mut expected_call = [0u8; 5];
    expected_call[0] = 0xe8;
    expected_call[1..].copy_from_slice(&displacement.to_le_bytes());
    if final_call_slice != expected_call || call_offset < wrapper_offset || call_end > wrapper_end {
        return Err(ProgramStorageEntryDiagnostic(
            "final program-storage wrapper call bytes do not target the exact Source interval"
                .into(),
        ));
    }
    Ok(expected_call)
}

#[cfg(test)]
mod tests {
    use super::validate_final_source_call;

    #[test]
    fn final_source_call_rejects_opcode_target_and_interval_tamper() {
        let wrapper_offset = 32;
        let wrapper_size = 16;
        let call_offset = 40;
        let continuation_offset = 8;
        let displacement = (continuation_offset as i32) - (call_offset as i32 + 5);
        let mut text = vec![0u8; 48];
        text[call_offset] = 0xe8;
        text[call_offset + 1..call_offset + 5].copy_from_slice(&displacement.to_le_bytes());
        assert_eq!(
            validate_final_source_call(
                &text,
                wrapper_offset,
                wrapper_size,
                continuation_offset,
                call_offset,
            )
            .expect("exact final rel32 call"),
            [
                0xe8,
                displacement.to_le_bytes()[0],
                displacement.to_le_bytes()[1],
                displacement.to_le_bytes()[2],
                displacement.to_le_bytes()[3],
            ]
        );

        let mut opcode_tamper = text.clone();
        opcode_tamper[call_offset] = 0x90;
        assert!(
            validate_final_source_call(
                &opcode_tamper,
                wrapper_offset,
                wrapper_size,
                continuation_offset,
                call_offset,
            )
            .is_err()
        );
        assert!(
            validate_final_source_call(
                &text,
                wrapper_offset,
                wrapper_size,
                continuation_offset + 1,
                call_offset,
            )
            .is_err()
        );
        assert!(
            validate_final_source_call(
                &text,
                wrapper_offset,
                12,
                continuation_offset,
                call_offset,
            )
            .is_err()
        );
    }
}

//! The report-only resolved linkage compatibility fingerprint.

use crate::dynamic_executable::file_assembly::dynamic_file_envelope::ValidatedElfDynamicFileEnvelope;
use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::candidates::ElfResolvedProcedureLinkageContents;
use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::{
    ElfAppliedProcedureLinkageTarget, FNV_OFFSET_BASIS, FNV_PRIME,
};

pub(crate) fn non_authoritative_resolved_linkage_compatibility_fingerprint(
    envelope: &ValidatedElfDynamicFileEnvelope,
    contents: &ElfResolvedProcedureLinkageContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf.resolved-procedure-linkage.v1");
    hash.bytes(
        &envelope
            .non_authoritative_envelope_compatibility_fingerprint()
            .to_le_bytes(),
    );
    hash.bytes(&contents.source_text_bytes);
    hash.bytes(&contents.procedure_linkage_bytes);
    hash.bytes(&contents.procedure_got_bytes);
    hash.bytes(&contents.procedure_relocation_bytes);
    hash.bytes(&(contents.applications.len() as u64).to_le_bytes());
    for application in &contents.applications {
        hash.bytes(&application.ordinal.to_le_bytes());
        hash.byte(application.storage as u8);
        hash.bytes(&(application.byte_offset as u64).to_le_bytes());
        hash.byte(application.byte_width);
        hash.bytes(&application.mutable_mask.to_le_bytes());
        hash.byte(application.kind as u8);
        hash_target(&mut hash, application.target);
        hash.bytes(&application.source_address.to_le_bytes());
        hash.bytes(&application.target_address.to_le_bytes());
        hash.bytes(&application.encoded_field.to_le_bytes());
    }
    hash.finish()
}

fn hash_target(hash: &mut Fnv1a, target: ElfAppliedProcedureLinkageTarget) {
    match target {
        ElfAppliedProcedureLinkageTarget::DynamicSection => hash.byte(1),
        ElfAppliedProcedureLinkageTarget::ProcedureLinkageHeader => hash.byte(2),
        ElfAppliedProcedureLinkageTarget::ProcedureLinkageEntry { logical_ordinal } => {
            hash.byte(3);
            hash.bytes(&logical_ordinal.to_le_bytes());
        }
        ElfAppliedProcedureLinkageTarget::ProcedureLinkageLazyTail { logical_ordinal } => {
            hash.byte(4);
            hash.bytes(&logical_ordinal.to_le_bytes());
        }
        ElfAppliedProcedureLinkageTarget::ProcedureGotHeaderWord { word_index } => {
            hash.byte(5);
            hash.byte(word_index);
        }
        ElfAppliedProcedureLinkageTarget::ProcedureGotSlot { logical_ordinal } => {
            hash.byte(6);
            hash.bytes(&logical_ordinal.to_le_bytes());
        }
        ElfAppliedProcedureLinkageTarget::RelocatedImageSection {
            section,
            byte_offset,
        } => {
            hash.byte(7);
            hash.byte(image_section_tag(section));
            hash.bytes(&(byte_offset as u64).to_le_bytes());
        }
    }
}

const fn image_section_tag(section: image::FinalImageSection) -> u8 {
    match section {
        image::FinalImageSection::Text => 1,
        image::FinalImageSection::Data => 2,
        image::FinalImageSection::Bss => 3,
        image::FinalImageSection::None => 4,
    }
}

struct Fnv1a(u64);

impl Fnv1a {
    const fn new() -> Self {
        Self(FNV_OFFSET_BASIS)
    }

    fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(FNV_PRIME);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            self.byte(byte);
        }
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

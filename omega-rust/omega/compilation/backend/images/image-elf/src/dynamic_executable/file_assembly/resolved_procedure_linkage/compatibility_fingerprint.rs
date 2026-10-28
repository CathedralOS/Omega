//! The report-only resolved linkage compatibility fingerprint.

use target::compatibility_fingerprint::Fnv1a;

use crate::dynamic_executable::file_assembly::dynamic_file_envelope::ValidatedElfDynamicFileEnvelope;
use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::ElfAppliedProcedureLinkageTarget;
use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::candidates::ElfResolvedProcedureLinkageContents;

pub(crate) fn non_authoritative_resolved_linkage_compatibility_fingerprint(
    envelope: &ValidatedElfDynamicFileEnvelope,
    contents: &ElfResolvedProcedureLinkageContents,
) -> u64 {
    let mut hash = crate::dynamic_executable::checked::stage_fingerprint(
        b"omega.elf.resolved-procedure-linkage.v1",
        envelope.non_authoritative_envelope_compatibility_fingerprint(),
    );
    hash.bytes(&contents.source_text_bytes);
    hash.bytes(&contents.source_data_bytes);
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
            hash.byte(crate::dynamic_executable::image_section_tag(section));
            hash.bytes(&(byte_offset as u64).to_le_bytes());
        }
    }
}

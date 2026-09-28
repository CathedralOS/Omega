//! The report-only template compatibility fingerprint.

use crate::dynamic_executable::procedure_linkage::dynamic_import_relocations::ValidatedElfProcedureLinkageRelocationPlan;
use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::template_plans::{
    ElfProcedureLinkageSemanticTarget, ElfProcedureLinkageTemplateContents,
};
use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::{
    FNV_OFFSET_BASIS, FNV_PRIME,
};

pub(crate) fn non_authoritative_template_compatibility_fingerprint(
    linkage: &ValidatedElfProcedureLinkageRelocationPlan,
    contents: &ElfProcedureLinkageTemplateContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf-procedure-linkage-templates.v1");
    hash.bytes(
        &linkage
            .non_authoritative_linkage_compatibility_fingerprint()
            .to_le_bytes(),
    );
    hash.byte(contents.policy as u8);
    hash.bytes(&contents.bytes.plt);
    hash.bytes(&contents.bytes.got_plt);
    hash.bytes(&contents.bytes.rela_plt);
    hash.bytes(&contents.bytes.rela_dyn);
    hash.bytes(&(contents.fixups.len() as u64).to_le_bytes());
    for fixup in &contents.fixups {
        hash.byte(fixup.storage as u8);
        hash.bytes(&(fixup.byte_offset as u64).to_le_bytes());
        hash.byte(fixup.byte_width);
        hash.bytes(&fixup.mutable_mask.to_le_bytes());
        hash.byte(fixup.kind as u8);
        hash_semantic_target(&mut hash, fixup.target);
    }
    hash.bytes(&(contents.constraints.len() as u64).to_le_bytes());
    for constraint in &contents.constraints {
        hash.bytes(&constraint.fixup_ordinal.to_le_bytes());
        hash.byte(constraint.kind as u8);
    }
    hash.finish()
}

fn hash_semantic_target(hash: &mut Fnv1a, target: ElfProcedureLinkageSemanticTarget) {
    match target {
        ElfProcedureLinkageSemanticTarget::FutureDynamicSection => hash.byte(1),
        ElfProcedureLinkageSemanticTarget::PltHeader => hash.byte(2),
        ElfProcedureLinkageSemanticTarget::PltEntry { logical_ordinal } => {
            hash.byte(3);
            hash.bytes(&logical_ordinal.to_le_bytes());
        }
        ElfProcedureLinkageSemanticTarget::PltLazyTail { logical_ordinal } => {
            hash.byte(4);
            hash.bytes(&logical_ordinal.to_le_bytes());
        }
        ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { word_index } => {
            hash.byte(5);
            hash.byte(word_index);
        }
        ElfProcedureLinkageSemanticTarget::GotPltSlot { logical_ordinal } => {
            hash.byte(6);
            hash.bytes(&logical_ordinal.to_le_bytes());
        }
        ElfProcedureLinkageSemanticTarget::RelocatedImageSection {
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

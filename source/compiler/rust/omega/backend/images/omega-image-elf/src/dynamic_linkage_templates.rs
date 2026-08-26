//! Target-specific address-free GOT/PLT and procedure-relocation templates.
//!
//! This module selects the baseline lazy-binding sequences specified by the
//! [x86-64 psABI] and the AArch64 [System V ABI]. It serializes only fixed
//! opcodes, ordinals, symbol/type coordinates, and zero placeholders. Every
//! placement-dependent field remains covered by a typed fixup and constraint;
//! these are templates, not final section payloads or runnable-image bytes.
//!
//! [x86-64 psABI]: https://gitlab.com/x86-psABIs/x86-64-ABI/-/blob/master/x86-64-ABI/dl.tex
//! [System V ABI]: https://github.com/ARM-software/abi-aa/blob/main/sysvabi64/sysvabi64.rst#procedure-linkage-table

use crate::bytes::write_u64;
use crate::dynamic_import_relocations::{
    ElfProcedureLinkageRelocationContents, ValidatedElfProcedureLinkageRelocationPlan,
};
use omega_target::TargetProfile;
use psi_diagnostics::Diagnostic;

const ELF64_RELA_SIZE: usize = 24;
const ELF64_GOT_WORD_SIZE: usize = 8;
const X86_PLT_HEADER_SIZE: usize = 16;
const X86_PLT_ENTRY_SIZE: usize = 16;
const AARCH64_PLT_HEADER_SIZE: usize = 32;
const AARCH64_PLT_ENTRY_SIZE: usize = 16;
const GOT_PLT_HEADER_WORDS: usize = 3;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

const X86_PLT_HEADER: [u8; X86_PLT_HEADER_SIZE] = [
    0xff, 0x35, 0, 0, 0, 0, // pushq GOT[1](%rip)
    0xff, 0x25, 0, 0, 0, 0, // jmpq *GOT[2](%rip)
    0x0f, 0x1f, 0x40, 0x00, // nopl 0(%rax)
];

const AARCH64_PLT_HEADER: [u8; AARCH64_PLT_HEADER_SIZE] = [
    0xf0, 0x7b, 0xbf, 0xa9, // stp x16, x30, [sp,#-16]!
    0x10, 0x00, 0x00, 0x90, // adrp x16, .got.plt[2]
    0x11, 0x02, 0x40, 0xf9, // ldr x17, [x16, :lo12:.got.plt[2]]
    0x10, 0x02, 0x00, 0x91, // add x16, x16, :lo12:.got.plt[2]
    0x20, 0x02, 0x1f, 0xd6, // br x17
    0x1f, 0x20, 0x03, 0xd5, // nop
    0x1f, 0x20, 0x03, 0xd5, // nop
    0x1f, 0x20, 0x03, 0xd5, // nop
];

const AARCH64_PLT_ENTRY: [u8; AARCH64_PLT_ENTRY_SIZE] = [
    0x10, 0x00, 0x00, 0x90, // adrp x16, .got.plt[N + 3]
    0x11, 0x02, 0x40, 0xf9, // ldr x17, [x16, :lo12:.got.plt[N + 3]]
    0x10, 0x02, 0x00, 0x91, // add x16, x16, :lo12:.got.plt[N + 3]
    0x20, 0x02, 0x1f, 0xd6, // br x17
];

/// Independently validated fixed template bytes and semantic fixups for the
/// exact address-free procedure-linkage plan.
///
/// The retained upstream plan remains the sole owner of import semantics and
/// source-call custody. No template field is a final address, section index,
/// placed byte, or image-mutation capability.
#[derive(Debug)]
#[must_use = "validated GOT/PLT templates retain the exact linkage plan"]
pub struct ValidatedElfProcedureLinkageTemplatePlan {
    linkage: ValidatedElfProcedureLinkageRelocationPlan,
    contents: ElfProcedureLinkageTemplateContents,
    template_identity: u64,
}

impl ValidatedElfProcedureLinkageTemplatePlan {
    pub const fn linkage(&self) -> &ValidatedElfProcedureLinkageRelocationPlan {
        &self.linkage
    }

    pub fn procedure_linkage_byte_count(&self) -> usize {
        self.contents.bytes.plt.len()
    }

    pub fn procedure_got_byte_count(&self) -> usize {
        self.contents.bytes.got_plt.len()
    }

    pub fn procedure_relocation_byte_count(&self) -> usize {
        self.contents.bytes.rela_plt.len()
    }

    pub fn fixup_count(&self) -> usize {
        self.contents.fixups.len()
    }

    pub fn placement_constraint_count(&self) -> usize {
        self.contents.constraints.len()
    }

    /// Compatibility fingerprint of the upstream linkage identity, exact
    /// target policy, fixed template bytes, semantic fixups, and deferred
    /// placement constraints. This is not final-byte or loader identity.
    pub const fn template_identity(&self) -> u64 {
        self.template_identity
    }

    #[allow(dead_code)]
    pub(crate) const fn contents(&self) -> &ElfProcedureLinkageTemplateContents {
        &self.contents
    }

    #[allow(dead_code)]
    pub(crate) fn into_parts(
        self,
    ) -> (
        ValidatedElfProcedureLinkageRelocationPlan,
        ElfProcedureLinkageTemplateContents,
    ) {
        (self.linkage, self.contents)
    }
}

/// Rejected GOT/PLT template planning with exact upstream custody.
#[derive(Debug)]
#[must_use = "ELF GOT/PLT template rejection retains the procedure-linkage plan"]
pub struct ElfProcedureLinkageTemplatePlanningError {
    linkage: ValidatedElfProcedureLinkageRelocationPlan,
    diagnostic: Diagnostic,
}

impl ElfProcedureLinkageTemplatePlanningError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfProcedureLinkageRelocationPlan, Diagnostic) {
        (self.linkage, self.diagnostic)
    }
}

impl std::fmt::Display for ElfProcedureLinkageTemplatePlanningError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfProcedureLinkageTemplatePlanningError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfProcedureLinkageTemplateContents {
    pub(crate) policy: ElfProcedureLinkageTemplatePolicy,
    pub(crate) bytes: ElfProcedureLinkageTemplateBytes,
    pub(crate) fixups: Vec<ElfProcedureLinkageFixup>,
    pub(crate) constraints: Vec<ElfProcedureLinkagePlacementConstraint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ElfProcedureLinkageTemplatePolicy {
    X86_64SmallMediumLazy = 1,
    Aarch64StandardLazy = 2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfProcedureLinkageTemplateBytes {
    pub(crate) plt: Vec<u8>,
    pub(crate) got_plt: Vec<u8>,
    pub(crate) rela_plt: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfProcedureLinkageFixup {
    pub(crate) storage: ElfProcedureLinkageFixupStorage,
    pub(crate) byte_offset: usize,
    pub(crate) byte_width: u8,
    pub(crate) mutable_mask: u64,
    pub(crate) kind: ElfProcedureLinkageFixupKind,
    pub(crate) target: ElfProcedureLinkageSemanticTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ElfProcedureLinkageFixupStorage {
    SourceText = 1,
    Plt = 2,
    GotPlt = 3,
    RelaPlt = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ElfProcedureLinkageFixupKind {
    X86PcRelative32 = 1,
    Aarch64Page21 = 2,
    Aarch64Load64Low12 = 3,
    Aarch64AddLow12 = 4,
    Aarch64Branch26 = 5,
    Absolute64 = 6,
    Elf64RelaOffset = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ElfProcedureLinkageSemanticTarget {
    FutureDynamicSection,
    PltHeader,
    PltEntry { logical_ordinal: u32 },
    PltLazyTail { logical_ordinal: u32 },
    GotPltHeaderWord { word_index: u8 },
    GotPltSlot { logical_ordinal: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfProcedureLinkagePlacementConstraint {
    pub(crate) fixup_ordinal: u32,
    pub(crate) kind: ElfProcedureLinkagePlacementConstraintKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ElfProcedureLinkagePlacementConstraintKind {
    X86Signed32 = 1,
    Aarch64PageDelta21 = 2,
    Aarch64Load64Low12Aligned = 3,
    Aarch64Branch26 = 4,
}

struct Candidate {
    linkage: ValidatedElfProcedureLinkageRelocationPlan,
    contents: ElfProcedureLinkageTemplateContents,
    template_identity: u64,
}

struct CandidateValidationError {
    candidate: Candidate,
    diagnostic: Diagnostic,
}

/// Consume exact semantic procedure linkage into fixed target template bytes,
/// typed fixups, and deferred placement constraints.
///
/// All placement-dependent fields remain exact zero placeholders. This does
/// not serialize final GOT/PLT/RELA payloads, resolve a fixup, assign a section
/// index or address, mutate the image, or grant runnable-image authority.
pub fn plan_elf_procedure_linkage_templates(
    linkage: ValidatedElfProcedureLinkageRelocationPlan,
) -> Result<ValidatedElfProcedureLinkageTemplatePlan, Box<ElfProcedureLinkageTemplatePlanningError>>
{
    let contents = match derive_contents(&linkage) {
        Ok(contents) => contents,
        Err(diagnostic) => {
            return Err(Box::new(ElfProcedureLinkageTemplatePlanningError {
                linkage,
                diagnostic,
            }));
        }
    };
    let template_identity = template_identity(&linkage, &contents);
    let candidate = Candidate {
        linkage,
        contents,
        template_identity,
    };
    match validate_candidate(candidate) {
        Ok(validated) => Ok(validated),
        Err(error) => Err(Box::new(ElfProcedureLinkageTemplatePlanningError {
            linkage: error.candidate.linkage,
            diagnostic: error.diagnostic,
        })),
    }
}

fn derive_contents(
    linkage: &ValidatedElfProcedureLinkageRelocationPlan,
) -> Result<ElfProcedureLinkageTemplateContents, Diagnostic> {
    let target = target(linkage);
    let policy = match target {
        TargetProfile::LinuxX64 => ElfProcedureLinkageTemplatePolicy::X86_64SmallMediumLazy,
        TargetProfile::LinuxArm64 => ElfProcedureLinkageTemplatePolicy::Aarch64StandardLazy,
        _ => {
            return Err(Diagnostic::error(
                "ELF GOT/PLT templates require an exact Linux x86-64 or AArch64 profile",
            ));
        }
    };
    let bytes = encode_template_bytes(target, linkage.contents())?;
    let fixups = derive_fixups(target, linkage.contents())?;
    let constraints = derive_constraints(&fixups)?;
    Ok(ElfProcedureLinkageTemplateContents {
        policy,
        bytes,
        fixups,
        constraints,
    })
}

fn target(linkage: &ValidatedElfProcedureLinkageRelocationPlan) -> TargetProfile {
    linkage
        .descriptors()
        .payloads()
        .plan()
        .inputs()
        .interpreter()
        .target()
}

fn encode_template_bytes(
    target: TargetProfile,
    linkage: &ElfProcedureLinkageRelocationContents,
) -> Result<ElfProcedureLinkageTemplateBytes, Diagnostic> {
    let slot_count = linkage.slots.len();
    let mut plt = match target {
        TargetProfile::LinuxX64 => Vec::with_capacity(checked_sum(
            X86_PLT_HEADER_SIZE,
            checked_product(slot_count, X86_PLT_ENTRY_SIZE, "x86-64 PLT entries")?,
            "x86-64 PLT template size",
        )?),
        TargetProfile::LinuxArm64 => Vec::with_capacity(checked_sum(
            AARCH64_PLT_HEADER_SIZE,
            checked_product(slot_count, AARCH64_PLT_ENTRY_SIZE, "AArch64 PLT entries")?,
            "AArch64 PLT template size",
        )?),
        _ => return Err(Diagnostic::error("unsupported ELF GOT/PLT template target")),
    };
    match target {
        TargetProfile::LinuxX64 => {
            plt.extend(X86_PLT_HEADER);
            for slot in &linkage.slots {
                let relocation_index = i32::try_from(slot.logical_ordinal).map_err(|_| {
                    Diagnostic::error("x86-64 lazy-binding relocation index exceeds signed imm32")
                })?;
                plt.extend([0xff, 0x25, 0, 0, 0, 0, 0x68]);
                plt.extend(relocation_index.to_le_bytes());
                plt.extend([0xe9, 0, 0, 0, 0]);
            }
        }
        TargetProfile::LinuxArm64 => {
            plt.extend(AARCH64_PLT_HEADER);
            for _ in &linkage.slots {
                plt.extend(AARCH64_PLT_ENTRY);
            }
        }
        _ => unreachable!("target was checked above"),
    }

    let got_word_count = checked_sum(GOT_PLT_HEADER_WORDS, slot_count, "procedure GOT word count")?;
    let got_plt = vec![
        0;
        checked_product(
            got_word_count,
            ELF64_GOT_WORD_SIZE,
            "procedure GOT template size",
        )?
    ];
    let mut rela_plt = Vec::with_capacity(checked_product(
        linkage.jump_slot_relocations.len(),
        ELF64_RELA_SIZE,
        "procedure RELA template size",
    )?);
    for relocation in &linkage.jump_slot_relocations {
        write_u64(&mut rela_plt, 0);
        let info = (u64::from(relocation.dynamic_symbol_index) << 32)
            | u64::from(relocation.relocation_type);
        write_u64(&mut rela_plt, info);
        write_u64(&mut rela_plt, relocation.addend as u64);
    }
    Ok(ElfProcedureLinkageTemplateBytes {
        plt,
        got_plt,
        rela_plt,
    })
}

fn derive_fixups(
    target: TargetProfile,
    linkage: &ElfProcedureLinkageRelocationContents,
) -> Result<Vec<ElfProcedureLinkageFixup>, Diagnostic> {
    let mut fixups = Vec::new();
    match target {
        TargetProfile::LinuxX64 => {
            push_fixup(
                &mut fixups,
                ElfProcedureLinkageFixupStorage::Plt,
                2,
                ElfProcedureLinkageFixupKind::X86PcRelative32,
                ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { word_index: 1 },
            );
            push_fixup(
                &mut fixups,
                ElfProcedureLinkageFixupStorage::Plt,
                8,
                ElfProcedureLinkageFixupKind::X86PcRelative32,
                ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { word_index: 2 },
            );
            for slot in &linkage.slots {
                let entry = checked_sum(
                    X86_PLT_HEADER_SIZE,
                    checked_product(
                        slot.logical_ordinal as usize,
                        X86_PLT_ENTRY_SIZE,
                        "x86-64 PLT entry offset",
                    )?,
                    "x86-64 PLT entry offset",
                )?;
                push_fixup(
                    &mut fixups,
                    ElfProcedureLinkageFixupStorage::Plt,
                    checked_sum(entry, 2, "x86-64 GOT displacement offset")?,
                    ElfProcedureLinkageFixupKind::X86PcRelative32,
                    ElfProcedureLinkageSemanticTarget::GotPltSlot {
                        logical_ordinal: slot.logical_ordinal,
                    },
                );
                push_fixup(
                    &mut fixups,
                    ElfProcedureLinkageFixupStorage::Plt,
                    checked_sum(entry, 12, "x86-64 PLT0 displacement offset")?,
                    ElfProcedureLinkageFixupKind::X86PcRelative32,
                    ElfProcedureLinkageSemanticTarget::PltHeader,
                );
            }
            push_fixup(
                &mut fixups,
                ElfProcedureLinkageFixupStorage::GotPlt,
                0,
                ElfProcedureLinkageFixupKind::Absolute64,
                ElfProcedureLinkageSemanticTarget::FutureDynamicSection,
            );
            for slot in &linkage.slots {
                push_fixup(
                    &mut fixups,
                    ElfProcedureLinkageFixupStorage::GotPlt,
                    got_slot_byte_offset(slot.logical_ordinal)?,
                    ElfProcedureLinkageFixupKind::Absolute64,
                    ElfProcedureLinkageSemanticTarget::PltLazyTail {
                        logical_ordinal: slot.logical_ordinal,
                    },
                );
            }
        }
        TargetProfile::LinuxArm64 => {
            for (offset, kind) in [
                (4, ElfProcedureLinkageFixupKind::Aarch64Page21),
                (8, ElfProcedureLinkageFixupKind::Aarch64Load64Low12),
                (12, ElfProcedureLinkageFixupKind::Aarch64AddLow12),
            ] {
                push_fixup(
                    &mut fixups,
                    ElfProcedureLinkageFixupStorage::Plt,
                    offset,
                    kind,
                    ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { word_index: 2 },
                );
            }
            for slot in &linkage.slots {
                let entry = checked_sum(
                    AARCH64_PLT_HEADER_SIZE,
                    checked_product(
                        slot.logical_ordinal as usize,
                        AARCH64_PLT_ENTRY_SIZE,
                        "AArch64 PLT entry offset",
                    )?,
                    "AArch64 PLT entry offset",
                )?;
                for (field, kind) in [
                    (0, ElfProcedureLinkageFixupKind::Aarch64Page21),
                    (4, ElfProcedureLinkageFixupKind::Aarch64Load64Low12),
                    (8, ElfProcedureLinkageFixupKind::Aarch64AddLow12),
                ] {
                    push_fixup(
                        &mut fixups,
                        ElfProcedureLinkageFixupStorage::Plt,
                        checked_sum(entry, field, "AArch64 PLT fixup offset")?,
                        kind,
                        ElfProcedureLinkageSemanticTarget::GotPltSlot {
                            logical_ordinal: slot.logical_ordinal,
                        },
                    );
                }
            }
            for slot in &linkage.slots {
                push_fixup(
                    &mut fixups,
                    ElfProcedureLinkageFixupStorage::GotPlt,
                    got_slot_byte_offset(slot.logical_ordinal)?,
                    ElfProcedureLinkageFixupKind::Absolute64,
                    ElfProcedureLinkageSemanticTarget::PltHeader,
                );
            }
        }
        _ => return Err(Diagnostic::error("unsupported ELF GOT/PLT fixup target")),
    }

    for (index, relocation) in linkage.jump_slot_relocations.iter().enumerate() {
        push_fixup(
            &mut fixups,
            ElfProcedureLinkageFixupStorage::RelaPlt,
            checked_product(index, ELF64_RELA_SIZE, "procedure RELA r_offset")?,
            ElfProcedureLinkageFixupKind::Elf64RelaOffset,
            ElfProcedureLinkageSemanticTarget::GotPltSlot {
                logical_ordinal: relocation.logical_got_slot_ordinal,
            },
        );
    }
    for slot in &linkage.slots {
        for call_site in &slot.call_sites {
            push_fixup(
                &mut fixups,
                ElfProcedureLinkageFixupStorage::SourceText,
                call_site.relocation_offset,
                match target {
                    TargetProfile::LinuxX64 => ElfProcedureLinkageFixupKind::X86PcRelative32,
                    TargetProfile::LinuxArm64 => ElfProcedureLinkageFixupKind::Aarch64Branch26,
                    _ => unreachable!("target was checked above"),
                },
                ElfProcedureLinkageSemanticTarget::PltEntry {
                    logical_ordinal: slot.logical_ordinal,
                },
            );
        }
    }
    Ok(fixups)
}

fn push_fixup(
    fixups: &mut Vec<ElfProcedureLinkageFixup>,
    storage: ElfProcedureLinkageFixupStorage,
    byte_offset: usize,
    kind: ElfProcedureLinkageFixupKind,
    target: ElfProcedureLinkageSemanticTarget,
) {
    let (byte_width, mutable_mask) = fixup_field(kind);
    fixups.push(ElfProcedureLinkageFixup {
        storage,
        byte_offset,
        byte_width,
        mutable_mask,
        kind,
        target,
    });
}

const fn fixup_field(kind: ElfProcedureLinkageFixupKind) -> (u8, u64) {
    match kind {
        ElfProcedureLinkageFixupKind::X86PcRelative32 => (4, 0xffff_ffff),
        ElfProcedureLinkageFixupKind::Aarch64Page21 => (4, 0x60ff_ffe0),
        ElfProcedureLinkageFixupKind::Aarch64Load64Low12
        | ElfProcedureLinkageFixupKind::Aarch64AddLow12 => (4, 0x003f_fc00),
        ElfProcedureLinkageFixupKind::Aarch64Branch26 => (4, 0x03ff_ffff),
        ElfProcedureLinkageFixupKind::Absolute64
        | ElfProcedureLinkageFixupKind::Elf64RelaOffset => (8, u64::MAX),
    }
}

fn derive_constraints(
    fixups: &[ElfProcedureLinkageFixup],
) -> Result<Vec<ElfProcedureLinkagePlacementConstraint>, Diagnostic> {
    let mut constraints = Vec::new();
    for (index, fixup) in fixups.iter().enumerate() {
        let kind = match fixup.kind {
            ElfProcedureLinkageFixupKind::X86PcRelative32 => {
                Some(ElfProcedureLinkagePlacementConstraintKind::X86Signed32)
            }
            ElfProcedureLinkageFixupKind::Aarch64Page21 => {
                Some(ElfProcedureLinkagePlacementConstraintKind::Aarch64PageDelta21)
            }
            ElfProcedureLinkageFixupKind::Aarch64Load64Low12 => {
                Some(ElfProcedureLinkagePlacementConstraintKind::Aarch64Load64Low12Aligned)
            }
            ElfProcedureLinkageFixupKind::Aarch64Branch26 => {
                Some(ElfProcedureLinkagePlacementConstraintKind::Aarch64Branch26)
            }
            ElfProcedureLinkageFixupKind::Aarch64AddLow12
            | ElfProcedureLinkageFixupKind::Absolute64
            | ElfProcedureLinkageFixupKind::Elf64RelaOffset => None,
        };
        if let Some(kind) = kind {
            constraints.push(ElfProcedureLinkagePlacementConstraint {
                fixup_ordinal: checked_u32(index, "procedure-linkage fixup ordinal")?,
                kind,
            });
        }
    }
    Ok(constraints)
}

fn got_slot_byte_offset(logical_ordinal: u32) -> Result<usize, Diagnostic> {
    checked_product(
        checked_sum(
            GOT_PLT_HEADER_WORDS,
            logical_ordinal as usize,
            "procedure GOT slot word index",
        )?,
        ELF64_GOT_WORD_SIZE,
        "procedure GOT slot byte offset",
    )
}

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfProcedureLinkageTemplatePlan, CandidateValidationError> {
    if let Err(diagnostic) = validate_contents(&candidate.linkage, &candidate.contents) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    if candidate.template_identity != template_identity(&candidate.linkage, &candidate.contents) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error("ELF GOT/PLT template identity does not replay"),
        });
    }
    Ok(ValidatedElfProcedureLinkageTemplatePlan {
        linkage: candidate.linkage,
        contents: candidate.contents,
        template_identity: candidate.template_identity,
    })
}

fn validate_contents(
    linkage: &ValidatedElfProcedureLinkageRelocationPlan,
    contents: &ElfProcedureLinkageTemplateContents,
) -> Result<(), Diagnostic> {
    let target = target(linkage);
    let expected_policy = match target {
        TargetProfile::LinuxX64 => ElfProcedureLinkageTemplatePolicy::X86_64SmallMediumLazy,
        TargetProfile::LinuxArm64 => ElfProcedureLinkageTemplatePolicy::Aarch64StandardLazy,
        _ => return Err(Diagnostic::error("unsupported validated GOT/PLT target")),
    };
    require(
        contents.policy == expected_policy,
        "ELF procedure-linkage template policy drifted from its exact target",
    )?;
    validate_fixed_template_bytes(target, linkage.contents(), &contents.bytes)?;
    let expected_fixups = derive_fixups(target, linkage.contents())?;
    require(
        contents.fixups == expected_fixups,
        "ELF procedure-linkage semantic fixups are missing, duplicated, or reordered",
    )?;
    validate_fixup_placeholders(linkage, contents)?;
    validate_fixup_coverage(&contents.fixups)?;
    require(
        contents.constraints == derive_constraints(&contents.fixups)?,
        "ELF procedure-linkage placement constraints drifted from exact fixups",
    )
}

fn validate_fixed_template_bytes(
    target: TargetProfile,
    linkage: &ElfProcedureLinkageRelocationContents,
    bytes: &ElfProcedureLinkageTemplateBytes,
) -> Result<(), Diagnostic> {
    let slot_count = linkage.slots.len();
    let expected_plt_size = match target {
        TargetProfile::LinuxX64 => checked_sum(
            X86_PLT_HEADER_SIZE,
            checked_product(
                slot_count,
                X86_PLT_ENTRY_SIZE,
                "validated x86-64 PLT entries",
            )?,
            "validated x86-64 PLT size",
        )?,
        TargetProfile::LinuxArm64 => checked_sum(
            AARCH64_PLT_HEADER_SIZE,
            checked_product(
                slot_count,
                AARCH64_PLT_ENTRY_SIZE,
                "validated AArch64 PLT entries",
            )?,
            "validated AArch64 PLT size",
        )?,
        _ => return Err(Diagnostic::error("unsupported fixed-template target")),
    };
    let expected_got_size = checked_product(
        checked_sum(GOT_PLT_HEADER_WORDS, slot_count, "validated GOT words")?,
        ELF64_GOT_WORD_SIZE,
        "validated GOT size",
    )?;
    let expected_rela_size = checked_product(
        linkage.jump_slot_relocations.len(),
        ELF64_RELA_SIZE,
        "validated procedure RELA size",
    )?;
    require(
        bytes.plt.len() == expected_plt_size
            && bytes.got_plt.len() == expected_got_size
            && bytes.rela_plt.len() == expected_rela_size,
        "ELF procedure-linkage template byte lengths do not match exact rows",
    )?;
    match target {
        TargetProfile::LinuxX64 => replay_x86_plt(linkage, &bytes.plt)?,
        TargetProfile::LinuxArm64 => replay_aarch64_plt(linkage, &bytes.plt)?,
        _ => unreachable!(),
    }
    require(
        bytes.got_plt.iter().all(|byte| *byte == 0),
        "ELF procedure GOT template contains a premature address or loader value",
    )?;
    replay_rela(linkage, &bytes.rela_plt)
}

fn replay_x86_plt(
    linkage: &ElfProcedureLinkageRelocationContents,
    bytes: &[u8],
) -> Result<(), Diagnostic> {
    require(
        bytes.get(..X86_PLT_HEADER_SIZE) == Some(&X86_PLT_HEADER),
        "x86-64 PLT0 template bytes drifted",
    )?;
    for (index, slot) in linkage.slots.iter().enumerate() {
        let start = checked_sum(
            X86_PLT_HEADER_SIZE,
            checked_product(index, X86_PLT_ENTRY_SIZE, "decoded x86-64 PLT entry")?,
            "decoded x86-64 PLT entry",
        )?;
        let end = checked_sum(start, X86_PLT_ENTRY_SIZE, "decoded x86-64 PLT entry end")?;
        let entry = bytes
            .get(start..end)
            .ok_or_else(|| Diagnostic::error("truncated x86-64 PLT entry"))?;
        require(
            entry.get(..7) == Some(&[0xff, 0x25, 0, 0, 0, 0, 0x68])
                && read_u32(entry, 7, "x86-64 PLT relocation ordinal")? == slot.logical_ordinal
                && entry.get(11..) == Some(&[0xe9, 0, 0, 0, 0]),
            "x86-64 PLT entry opcode, ordinal, or placeholder drifted",
        )?;
    }
    Ok(())
}

fn replay_aarch64_plt(
    linkage: &ElfProcedureLinkageRelocationContents,
    bytes: &[u8],
) -> Result<(), Diagnostic> {
    require(
        bytes.get(..AARCH64_PLT_HEADER_SIZE) == Some(&AARCH64_PLT_HEADER),
        "AArch64 PLT0 template bytes drifted",
    )?;
    for index in 0..linkage.slots.len() {
        let start = checked_sum(
            AARCH64_PLT_HEADER_SIZE,
            checked_product(index, AARCH64_PLT_ENTRY_SIZE, "decoded AArch64 PLT entry")?,
            "decoded AArch64 PLT entry",
        )?;
        let end = checked_sum(
            start,
            AARCH64_PLT_ENTRY_SIZE,
            "decoded AArch64 PLT entry end",
        )?;
        require(
            bytes.get(start..end) == Some(&AARCH64_PLT_ENTRY),
            "AArch64 PLT entry opcode or zero placeholder drifted",
        )?;
    }
    Ok(())
}

fn replay_rela(
    linkage: &ElfProcedureLinkageRelocationContents,
    bytes: &[u8],
) -> Result<(), Diagnostic> {
    for (index, relocation) in linkage.jump_slot_relocations.iter().enumerate() {
        let start = checked_product(index, ELF64_RELA_SIZE, "decoded procedure RELA row")?;
        let expected_info = (u64::from(relocation.dynamic_symbol_index) << 32)
            | u64::from(relocation.relocation_type);
        require(
            read_u64(bytes, start, "procedure RELA r_offset")? == 0
                && read_u64(
                    bytes,
                    checked_sum(start, 8, "procedure RELA r_info offset")?,
                    "procedure RELA r_info",
                )? == expected_info
                && read_u64(
                    bytes,
                    checked_sum(start, 16, "procedure RELA addend offset")?,
                    "procedure RELA addend",
                )? == relocation.addend as u64,
            "ELF procedure RELA fixed row or address placeholder drifted",
        )?;
    }
    Ok(())
}

fn validate_fixup_placeholders(
    linkage: &ValidatedElfProcedureLinkageRelocationPlan,
    contents: &ElfProcedureLinkageTemplateContents,
) -> Result<(), Diagnostic> {
    let source_text = &linkage
        .descriptors()
        .payloads()
        .plan()
        .inputs()
        .image()
        .memory
        .text;
    for fixup in &contents.fixups {
        let storage = match fixup.storage {
            ElfProcedureLinkageFixupStorage::SourceText => source_text,
            ElfProcedureLinkageFixupStorage::Plt => &contents.bytes.plt,
            ElfProcedureLinkageFixupStorage::GotPlt => &contents.bytes.got_plt,
            ElfProcedureLinkageFixupStorage::RelaPlt => &contents.bytes.rela_plt,
        };
        let field = read_field(storage, fixup.byte_offset, fixup.byte_width)?;
        require(
            field & fixup.mutable_mask == 0,
            "ELF procedure-linkage fixup field is not an exact zero placeholder",
        )?;
        require(
            fixup_field(fixup.kind) == (fixup.byte_width, fixup.mutable_mask),
            "ELF procedure-linkage fixup width or mutable mask drifted from its kind",
        )?;
        validate_semantic_target(linkage.contents(), fixup.target)?;
    }
    Ok(())
}

fn validate_semantic_target(
    linkage: &ElfProcedureLinkageRelocationContents,
    target: ElfProcedureLinkageSemanticTarget,
) -> Result<(), Diagnostic> {
    let valid_ordinal = |ordinal: u32| {
        linkage
            .slots
            .get(ordinal as usize)
            .is_some_and(|slot| slot.logical_ordinal == ordinal)
    };
    require(
        match target {
            ElfProcedureLinkageSemanticTarget::FutureDynamicSection
            | ElfProcedureLinkageSemanticTarget::PltHeader => true,
            ElfProcedureLinkageSemanticTarget::PltEntry { logical_ordinal }
            | ElfProcedureLinkageSemanticTarget::PltLazyTail { logical_ordinal }
            | ElfProcedureLinkageSemanticTarget::GotPltSlot { logical_ordinal } => {
                valid_ordinal(logical_ordinal)
            }
            ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { word_index } => {
                usize::from(word_index) < GOT_PLT_HEADER_WORDS
            }
        },
        "ELF procedure-linkage fixup target does not resolve semantically",
    )
}

fn validate_fixup_coverage(fixups: &[ElfProcedureLinkageFixup]) -> Result<(), Diagnostic> {
    for (index, fixup) in fixups.iter().enumerate() {
        for other in &fixups[index + 1..] {
            if fixup.storage != other.storage {
                continue;
            }
            let end = checked_sum(
                fixup.byte_offset,
                usize::from(fixup.byte_width),
                "procedure-linkage fixup end",
            )?;
            let other_end = checked_sum(
                other.byte_offset,
                usize::from(other.byte_width),
                "procedure-linkage fixup end",
            )?;
            require(
                end <= other.byte_offset || other_end <= fixup.byte_offset,
                "ELF procedure-linkage fixups overlap or duplicate one field",
            )?;
        }
    }
    Ok(())
}

fn read_field(bytes: &[u8], offset: usize, width: u8) -> Result<u64, Diagnostic> {
    match width {
        4 => Ok(u64::from(read_u32(bytes, offset, "32-bit fixup field")?)),
        8 => read_u64(bytes, offset, "64-bit fixup field"),
        _ => Err(Diagnostic::error(
            "ELF procedure-linkage fixup has an unsupported field width",
        )),
    }
}

fn read_u32(bytes: &[u8], offset: usize, context: &'static str) -> Result<u32, Diagnostic> {
    let end = checked_sum(offset, 4, context)?;
    let value = bytes
        .get(offset..end)
        .and_then(|value| value.try_into().ok())
        .ok_or_else(|| Diagnostic::error(format!("truncated {context}")))?;
    Ok(u32::from_le_bytes(value))
}

fn read_u64(bytes: &[u8], offset: usize, context: &'static str) -> Result<u64, Diagnostic> {
    let end = checked_sum(offset, 8, context)?;
    let value = bytes
        .get(offset..end)
        .and_then(|value| value.try_into().ok())
        .ok_or_else(|| Diagnostic::error(format!("truncated {context}")))?;
    Ok(u64::from_le_bytes(value))
}

fn checked_product(left: usize, right: usize, context: &'static str) -> Result<usize, Diagnostic> {
    left.checked_mul(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows usize")))
}

fn checked_sum(left: usize, right: usize, context: &'static str) -> Result<usize, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows usize")))
}

fn checked_u32(value: usize, context: &'static str) -> Result<u32, Diagnostic> {
    u32::try_from(value).map_err(|_| Diagnostic::error(format!("{context} exceeds Elf64_Word")))
}

fn require(condition: bool, message: &'static str) -> Result<(), Diagnostic> {
    condition
        .then_some(())
        .ok_or_else(|| Diagnostic::error(message))
}

fn template_identity(
    linkage: &ValidatedElfProcedureLinkageRelocationPlan,
    contents: &ElfProcedureLinkageTemplateContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf-procedure-linkage-templates.v1");
    hash.bytes(&linkage.linkage_identity().to_le_bytes());
    hash.byte(contents.policy as u8);
    hash.bytes(&contents.bytes.plt);
    hash.bytes(&contents.bytes.got_plt);
    hash.bytes(&contents.bytes.rela_plt);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        plan_elf_dynamic_link_inputs, plan_elf_dynamic_section_descriptors,
        plan_elf_dynamic_sections, plan_elf_procedure_linkage_relocations,
        serialize_elf_dynamic_sections,
    };
    use omega_image::{
        FinalImage, FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
        FinalImageSection, FinalImageSymbol,
    };
    use omega_object_file::{RelocationKind, SymbolKind};
    use omega_target::{
        ForeignLocatorCandidate, normalize_elf_interpreter_plan, normalize_foreign_locator,
    };
    use psi_arena::Handle;

    #[derive(Clone, Copy)]
    struct ImportFixture {
        object: &'static [u8],
        symbol: &'static [u8],
        version: &'static [u8],
        instruction_offsets: &'static [usize],
    }

    const FIRST_SITES: &[usize] = &[0, 44];
    const SECOND_SITES: &[usize] = &[16];
    const THIRD_SITES: &[usize] = &[28];
    const IMPORTS: [ImportFixture; 3] = [
        ImportFixture {
            object: b"liba\xff.so",
            symbol: b"alpha\xfe",
            version: b"V1\xfd",
            instruction_offsets: FIRST_SITES,
        },
        ImportFixture {
            object: b"liba\xff.so",
            symbol: b"beta",
            version: b"V2",
            instruction_offsets: SECOND_SITES,
        },
        ImportFixture {
            object: b"libb.so",
            symbol: b"gamma",
            version: b"V1\xfd",
            instruction_offsets: THIRD_SITES,
        },
    ];

    fn interpreter_path(target: TargetProfile) -> &'static [u8] {
        match target {
            TargetProfile::LinuxX64 => b"/lib64/ld-linux-\xfc-x86-64.so.2",
            TargetProfile::LinuxArm64 => b"/lib/ld-linux-\xfb-aarch64.so.1",
            _ => unreachable!("template fixture uses a Linux target"),
        }
    }

    fn linkage(
        target: TargetProfile,
        imports: &[ImportFixture],
    ) -> ValidatedElfProcedureLinkageRelocationPlan {
        let relocation_count = imports
            .iter()
            .map(|fixture| fixture.instruction_offsets.len())
            .sum();
        let mut image = FinalImage::with_capacity(
            target.native_target(),
            FinalImageMemory {
                text: vec![0; 64],
                ..FinalImageMemory::default()
            },
            Handle::invalid(),
            imports.len(),
            imports.len(),
            relocation_count,
        );
        for (index, fixture) in imports.iter().enumerate() {
            let symbol_handle = image.symbol_table.symbols.insert(FinalImageSymbol {
                name: format!("__omega_template_import_{index}"),
                section: FinalImageSection::None,
                offset: 0,
                size: 0,
                kind: SymbolKind::Import,
            });
            image.symbol_table.imports.insert(FinalImageImport {
                symbol_handle,
                import: FinalImageImportPlan::Normalized(
                    normalize_foreign_locator(
                        ForeignLocatorCandidate::ElfVersioned {
                            object: fixture.object.to_vec(),
                            symbol: fixture.symbol.to_vec(),
                            version: fixture.version.to_vec(),
                        },
                        target,
                    )
                    .expect("valid template locator"),
                ),
            });
            for instruction_offset in fixture.instruction_offsets {
                let (relocation_offset, kind) = match target {
                    TargetProfile::LinuxX64 => {
                        image.memory.text[*instruction_offset] = 0xe8;
                        (instruction_offset + 1, RelocationKind::X86_64Relative32)
                    }
                    TargetProfile::LinuxArm64 => {
                        image.memory.text[*instruction_offset..*instruction_offset + 4]
                            .copy_from_slice(&[0, 0, 0, 0x94]);
                        (*instruction_offset, RelocationKind::Aarch64Branch26)
                    }
                    _ => unreachable!(),
                };
                image
                    .relocation_table
                    .relocations
                    .insert(FinalImageRelocation {
                        section: FinalImageSection::Text,
                        offset: relocation_offset,
                        byte_width: 4,
                        symbol_handle,
                        addend: 0,
                        kind,
                    });
            }
        }
        let interpreter = normalize_elf_interpreter_plan(interpreter_path(target).to_vec(), target)
            .expect("valid template interpreter");
        let inputs =
            plan_elf_dynamic_link_inputs(image, interpreter).expect("valid dynamic-link inputs");
        let sections = plan_elf_dynamic_sections(inputs).expect("valid dynamic sections");
        let payloads = serialize_elf_dynamic_sections(sections).expect("valid dynamic payloads");
        let descriptors =
            plan_elf_dynamic_section_descriptors(payloads).expect("valid dynamic descriptors");
        plan_elf_procedure_linkage_relocations(descriptors)
            .expect("valid semantic procedure linkage")
    }

    fn candidate(target: TargetProfile) -> Candidate {
        let linkage = linkage(target, &IMPORTS);
        let contents = derive_contents(&linkage).expect("derived templates");
        let template_identity = template_identity(&linkage, &contents);
        Candidate {
            linkage,
            contents,
            template_identity,
        }
    }

    #[test]
    fn both_targets_emit_exact_fixed_templates_fixups_and_constraints() {
        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let plan = plan_elf_procedure_linkage_templates(linkage(target, &IMPORTS))
                .expect("validated linkage templates");
            assert_eq!(plan.procedure_got_byte_count(), 48);
            assert_eq!(plan.procedure_relocation_byte_count(), 72);
            assert_eq!(
                plan.procedure_linkage_byte_count(),
                match target {
                    TargetProfile::LinuxX64 => 64,
                    TargetProfile::LinuxArm64 => 80,
                    _ => unreachable!(),
                }
            );
            assert_eq!(
                plan.fixup_count(),
                match target {
                    TargetProfile::LinuxX64 => 19,
                    TargetProfile::LinuxArm64 => 22,
                    _ => unreachable!(),
                }
            );
            assert_eq!(plan.placement_constraint_count(), 12);
            assert_ne!(plan.template_identity(), 0);
            assert!(plan.contents.bytes.got_plt.iter().all(|byte| *byte == 0));

            for (index, relocation) in plan
                .linkage
                .contents()
                .jump_slot_relocations
                .iter()
                .enumerate()
            {
                let start = index * ELF64_RELA_SIZE;
                assert_eq!(
                    read_u64(&plan.contents.bytes.rela_plt, start, "offset").unwrap(),
                    0
                );
                assert_eq!(
                    read_u64(&plan.contents.bytes.rela_plt, start + 8, "info").unwrap(),
                    (u64::from(relocation.dynamic_symbol_index) << 32)
                        | u64::from(relocation.relocation_type),
                );
                assert_eq!(
                    read_u64(&plan.contents.bytes.rela_plt, start + 16, "addend").unwrap(),
                    0,
                );
            }
            validate_contents(plan.linkage(), &plan.contents).expect("independent template replay");
        }
    }

    #[test]
    fn x86_uses_dynamic_got_zero_and_exact_small_medium_lazy_entries() {
        let plan = plan_elf_procedure_linkage_templates(linkage(TargetProfile::LinuxX64, &IMPORTS))
            .expect("validated x86 templates");
        assert_eq!(&plan.contents.bytes.plt[..16], &X86_PLT_HEADER);
        assert_eq!(
            &plan.contents.bytes.plt[16..32],
            &[0xff, 0x25, 0, 0, 0, 0, 0x68, 0, 0, 0, 0, 0xe9, 0, 0, 0, 0],
        );
        assert_eq!(
            &plan.contents.bytes.plt[32..48],
            &[0xff, 0x25, 0, 0, 0, 0, 0x68, 1, 0, 0, 0, 0xe9, 0, 0, 0, 0],
        );
        assert!(plan.contents.fixups.contains(&ElfProcedureLinkageFixup {
            storage: ElfProcedureLinkageFixupStorage::GotPlt,
            byte_offset: 0,
            byte_width: 8,
            mutable_mask: u64::MAX,
            kind: ElfProcedureLinkageFixupKind::Absolute64,
            target: ElfProcedureLinkageSemanticTarget::FutureDynamicSection,
        }));
        assert!(!plan.contents.fixups.iter().any(|fixup| {
            fixup.storage == ElfProcedureLinkageFixupStorage::GotPlt
                && matches!(fixup.byte_offset, 8 | 16)
        }));
    }

    #[test]
    fn aarch64_keeps_all_three_header_words_zero_and_targets_plt_zero() {
        let plan =
            plan_elf_procedure_linkage_templates(linkage(TargetProfile::LinuxArm64, &IMPORTS))
                .expect("validated AArch64 templates");
        assert_eq!(&plan.contents.bytes.plt[..32], &AARCH64_PLT_HEADER);
        assert_eq!(&plan.contents.bytes.plt[32..48], &AARCH64_PLT_ENTRY);
        assert_eq!(&plan.contents.bytes.got_plt[..24], &[0; 24]);
        assert!(!plan.contents.fixups.iter().any(|fixup| {
            fixup.storage == ElfProcedureLinkageFixupStorage::GotPlt && fixup.byte_offset < 24
        }));
        assert_eq!(
            plan.contents
                .fixups
                .iter()
                .filter(|fixup| {
                    fixup.storage == ElfProcedureLinkageFixupStorage::GotPlt
                        && matches!(fixup.target, ElfProcedureLinkageSemanticTarget::PltHeader)
                })
                .count(),
            3,
        );
        assert!(!plan.contents.fixups.iter().any(|fixup| matches!(
            fixup.target,
            ElfProcedureLinkageSemanticTarget::FutureDynamicSection
        )));
    }

    #[test]
    fn import_permutation_and_repeated_calls_preserve_canonical_template_identity() {
        let forward =
            plan_elf_procedure_linkage_templates(linkage(TargetProfile::LinuxX64, &IMPORTS))
                .unwrap();
        let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
        let reverse = plan_elf_procedure_linkage_templates(linkage(
            TargetProfile::LinuxX64,
            &reverse_imports,
        ))
        .unwrap();
        let arm =
            plan_elf_procedure_linkage_templates(linkage(TargetProfile::LinuxArm64, &IMPORTS))
                .unwrap();

        assert_eq!(forward.template_identity(), reverse.template_identity());
        assert_eq!(forward.contents, reverse.contents);
        assert_ne!(forward.template_identity(), arm.template_identity());
        assert_eq!(forward.linkage().logical_slot_count(), 3);
        assert_eq!(forward.linkage().direct_call_site_count(), 4);
    }

    #[test]
    fn independent_replay_rejects_every_template_fixup_constraint_and_identity_corruption() {
        let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
            Box::new(|candidate| {
                candidate.contents.policy = ElfProcedureLinkageTemplatePolicy::Aarch64StandardLazy
            }),
            Box::new(|candidate| candidate.contents.bytes.plt[0] ^= 1),
            Box::new(|candidate| candidate.contents.bytes.plt[2] = 1),
            Box::new(|candidate| {
                candidate.contents.bytes.plt.pop();
            }),
            Box::new(|candidate| candidate.contents.bytes.plt.push(0)),
            Box::new(|candidate| candidate.contents.bytes.got_plt[0] = 1),
            Box::new(|candidate| {
                candidate.contents.bytes.got_plt.pop();
            }),
            Box::new(|candidate| candidate.contents.bytes.got_plt.push(0)),
            Box::new(|candidate| candidate.contents.bytes.rela_plt[0] = 1),
            Box::new(|candidate| candidate.contents.bytes.rela_plt[8] ^= 1),
            Box::new(|candidate| candidate.contents.bytes.rela_plt[16] = 1),
            Box::new(|candidate| {
                candidate.contents.bytes.rela_plt.pop();
            }),
            Box::new(|candidate| candidate.contents.bytes.rela_plt.push(0)),
            Box::new(|candidate| {
                candidate.contents.fixups.pop();
            }),
            Box::new(|candidate| candidate.contents.fixups.push(candidate.contents.fixups[0])),
            Box::new(|candidate| candidate.contents.fixups.swap(0, 1)),
            Box::new(|candidate| {
                candidate.contents.fixups[0].storage = ElfProcedureLinkageFixupStorage::RelaPlt
            }),
            Box::new(|candidate| candidate.contents.fixups[0].byte_offset = usize::MAX),
            Box::new(|candidate| candidate.contents.fixups[0].byte_width = 8),
            Box::new(|candidate| candidate.contents.fixups[0].mutable_mask ^= 1),
            Box::new(|candidate| {
                candidate.contents.fixups[0].kind = ElfProcedureLinkageFixupKind::Aarch64Page21
            }),
            Box::new(|candidate| {
                candidate.contents.fixups[0].target =
                    ElfProcedureLinkageSemanticTarget::GotPltSlot {
                        logical_ordinal: u32::MAX,
                    }
            }),
            Box::new(|candidate| {
                candidate.contents.constraints.pop();
            }),
            Box::new(|candidate| candidate.contents.constraints[0].fixup_ordinal = u32::MAX),
            Box::new(|candidate| {
                candidate.contents.constraints[0].kind =
                    ElfProcedureLinkagePlacementConstraintKind::Aarch64Branch26
            }),
            Box::new(|candidate| candidate.template_identity ^= 1),
        ];

        for corrupt in corruptions {
            let mut candidate = candidate(TargetProfile::LinuxX64);
            let expected_identity = candidate.linkage.linkage_identity();
            corrupt(&mut candidate);
            let error = validate_candidate(candidate)
                .expect_err("corrupt linkage template candidate must reject");
            assert_eq!(
                error.candidate.linkage.linkage_identity(),
                expected_identity,
                "template rejection retains exact linkage custody",
            );
        }
    }

    #[test]
    fn aarch64_opcode_placeholder_and_reserved_header_corruption_rejects() {
        let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
            Box::new(|candidate| candidate.contents.bytes.plt[0] ^= 1),
            Box::new(|candidate| candidate.contents.bytes.plt[4] |= 0x20),
            Box::new(|candidate| candidate.contents.bytes.plt[8] |= 0x04),
            Box::new(|candidate| candidate.contents.bytes.plt[12] |= 0x04),
            Box::new(|candidate| candidate.contents.bytes.plt[32] |= 0x20),
            Box::new(|candidate| candidate.contents.bytes.got_plt[0] = 1),
            Box::new(|candidate| candidate.contents.bytes.got_plt[8] = 1),
            Box::new(|candidate| candidate.contents.bytes.got_plt[16] = 1),
            Box::new(|candidate| {
                candidate.contents.fixups[0].target =
                    ElfProcedureLinkageSemanticTarget::FutureDynamicSection
            }),
        ];
        for corrupt in corruptions {
            let mut candidate = candidate(TargetProfile::LinuxArm64);
            let expected_identity = candidate.linkage.linkage_identity();
            corrupt(&mut candidate);
            let error =
                validate_candidate(candidate).expect_err("corrupt AArch64 template must reject");
            assert_eq!(
                error.candidate.linkage.linkage_identity(),
                expected_identity
            );
        }
    }

    #[test]
    fn malformed_lengths_offsets_masks_and_arithmetic_reject_without_panicking() {
        assert!(checked_sum(usize::MAX, 1, "sum").is_err());
        assert!(checked_product(usize::MAX, 2, "product").is_err());
        assert!(checked_u32(usize::MAX, "word").is_err());
        assert!(read_u32(&[], usize::MAX, "u32").is_err());
        assert!(read_u64(&[0; 7], 0, "u64").is_err());
        assert!(read_field(&[0; 8], 0, 3).is_err());

        let overflowing = ElfProcedureLinkageFixup {
            storage: ElfProcedureLinkageFixupStorage::Plt,
            byte_offset: usize::MAX,
            byte_width: 8,
            mutable_mask: u64::MAX,
            kind: ElfProcedureLinkageFixupKind::Absolute64,
            target: ElfProcedureLinkageSemanticTarget::PltHeader,
        };
        assert!(validate_fixup_coverage(&[overflowing, overflowing]).is_err());
    }
}

/*
We drive the 22-stage dynamic ELF executable pipeline
(`emit_elf_dynamic_executable`), chaining the six phase modules from
dynamic-link input preflight through final-byte admission and wrapping any
stage failure in `ElfDynamicExecutableEmissionError` alongside the shared
`checked` validation helpers.
*/

pub(crate) mod checked {
    use diagnostics::Diagnostic;

    use crate::constants::{
        ELF64_DYNAMIC_ROW_SIZE, ELF64_DYNAMIC_VALUE_OFFSET, ELF64_SECTION_HEADER_SIZE,
    };
    use target::compatibility_fingerprint::Fnv1a;

    pub(crate) fn require(condition: bool, message: &'static str) -> Result<(), Diagnostic> {
        condition
            .then_some(())
            .ok_or_else(|| Diagnostic::error(message))
    }

    pub(crate) fn checked_sum(
        left: usize,
        right: usize,
        context: &'static str,
    ) -> Result<usize, Diagnostic> {
        left.checked_add(right)
            .ok_or_else(|| Diagnostic::error(format!("{context} overflows usize")))
    }

    pub(crate) fn checked_product(
        left: usize,
        right: usize,
        context: &'static str,
    ) -> Result<usize, Diagnostic> {
        left.checked_mul(right)
            .ok_or_else(|| Diagnostic::error(format!("{context} overflows usize")))
    }

    pub(crate) fn checked_u32(value: usize, context: &'static str) -> Result<u32, Diagnostic> {
        u32::try_from(value).map_err(|_| Diagnostic::error(format!("{context} exceeds Elf64_Word")))
    }

    fn read_array<const N: usize>(
        bytes: &[u8],
        offset: usize,
        context: &'static str,
    ) -> Result<[u8; N], Diagnostic> {
        let end = checked_sum(offset, N, context)?;
        bytes
            .get(offset..end)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(|| Diagnostic::error(format!("truncated {context}")))
    }

    pub(crate) fn read_u8(
        bytes: &[u8],
        offset: usize,
        context: &'static str,
    ) -> Result<u8, Diagnostic> {
        bytes
            .get(offset)
            .copied()
            .ok_or_else(|| Diagnostic::error(format!("truncated {context}")))
    }

    pub(crate) fn read_u16(
        bytes: &[u8],
        offset: usize,
        context: &'static str,
    ) -> Result<u16, Diagnostic> {
        Ok(u16::from_le_bytes(read_array::<2>(bytes, offset, context)?))
    }

    pub(crate) fn read_u32(
        bytes: &[u8],
        offset: usize,
        context: &'static str,
    ) -> Result<u32, Diagnostic> {
        Ok(u32::from_le_bytes(read_array::<4>(bytes, offset, context)?))
    }

    pub(crate) fn read_u64(
        bytes: &[u8],
        offset: usize,
        context: &'static str,
    ) -> Result<u64, Diagnostic> {
        Ok(u64::from_le_bytes(read_array::<8>(bytes, offset, context)?))
    }

    pub(crate) fn read_i64(
        bytes: &[u8],
        offset: usize,
        context: &'static str,
    ) -> Result<i64, Diagnostic> {
        Ok(i64::from_le_bytes(read_array::<8>(bytes, offset, context)?))
    }

    pub(crate) fn read_field(
        bytes: &[u8],
        offset: usize,
        width: u8,
        context: &'static str,
    ) -> Result<u64, Diagnostic> {
        match width {
            4 => Ok(u64::from(read_u32(bytes, offset, context)?)),
            8 => read_u64(bytes, offset, context),
            _ => Err(Diagnostic::error(format!(
                "{context} has an unsupported field width"
            ))),
        }
    }

    pub(crate) fn field_mut<'a>(
        bytes: &'a mut [u8],
        offset: usize,
        width: u8,
        context: &'static str,
    ) -> Result<&'a mut [u8], Diagnostic> {
        let end = checked_sum(offset, usize::from(width), context)?;
        bytes
            .get_mut(offset..end)
            .ok_or_else(|| Diagnostic::error(format!("{context} exceeds its storage")))
    }

    pub(crate) fn stage_fingerprint(tag: &[u8], upstream: u64) -> Fnv1a {
        let mut hash = Fnv1a::new();
        hash.bytes(tag);
        hash.bytes(&upstream.to_le_bytes());
        hash
    }

    pub(crate) fn checked_sum_u64(
        left: u64,
        right: u64,
        context: &str,
        bound: &'static str,
    ) -> Result<u64, Diagnostic> {
        left.checked_add(right)
            .ok_or_else(|| Diagnostic::error(format!("{context} overflows {bound}")))
    }

    pub(crate) fn checked_product_u64(
        left: u64,
        right: u64,
        context: &str,
        bound: &'static str,
    ) -> Result<u64, Diagnostic> {
        left.checked_mul(right)
            .ok_or_else(|| Diagnostic::error(format!("{context} overflows {bound}")))
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct DecodedElf64SectionHeader {
        pub(crate) name_offset: u32,
        pub(crate) section_type: u32,
        pub(crate) flags: u64,
        pub(crate) address: u64,
        pub(crate) file_offset: u64,
        pub(crate) payload_size: u64,
        pub(crate) link: u32,
        pub(crate) info: u32,
        pub(crate) alignment: u64,
        pub(crate) entry_size: u64,
    }

    pub(crate) fn decode_section_header_rows(
        bytes: &[u8],
        expected_count: usize,
    ) -> Result<Vec<DecodedElf64SectionHeader>, Diagnostic> {
        let expected_size = checked_product(
            expected_count,
            ELF64_SECTION_HEADER_SIZE,
            "decoded ELF64 section-header table size",
        )?;
        require(
            bytes.len() == expected_size,
            "ELF64 section-header table has a truncated row or trailing bytes",
        )?;
        let mut rows = Vec::with_capacity(expected_count);
        for index in 0..expected_count {
            let offset = checked_product(
                index,
                ELF64_SECTION_HEADER_SIZE,
                "decoded ELF64 section-header row",
            )?;
            let field = |field_offset: usize, context: &'static str| {
                checked_sum(offset, field_offset, context)
            };
            rows.push(DecodedElf64SectionHeader {
                name_offset: read_u32(bytes, offset, "Elf64_Shdr.sh_name")?,
                section_type: read_u32(
                    bytes,
                    field(4, "Elf64_Shdr.sh_type offset")?,
                    "Elf64_Shdr.sh_type",
                )?,
                flags: read_u64(
                    bytes,
                    field(8, "Elf64_Shdr.sh_flags offset")?,
                    "Elf64_Shdr.sh_flags",
                )?,
                address: read_u64(
                    bytes,
                    field(16, "Elf64_Shdr.sh_addr offset")?,
                    "Elf64_Shdr.sh_addr",
                )?,
                file_offset: read_u64(
                    bytes,
                    field(24, "Elf64_Shdr.sh_offset offset")?,
                    "Elf64_Shdr.sh_offset",
                )?,
                payload_size: read_u64(
                    bytes,
                    field(32, "Elf64_Shdr.sh_size offset")?,
                    "Elf64_Shdr.sh_size",
                )?,
                link: read_u32(
                    bytes,
                    field(40, "Elf64_Shdr.sh_link offset")?,
                    "Elf64_Shdr.sh_link",
                )?,
                info: read_u32(
                    bytes,
                    field(44, "Elf64_Shdr.sh_info offset")?,
                    "Elf64_Shdr.sh_info",
                )?,
                alignment: read_u64(
                    bytes,
                    field(48, "Elf64_Shdr.sh_addralign offset")?,
                    "Elf64_Shdr.sh_addralign",
                )?,
                entry_size: read_u64(
                    bytes,
                    field(56, "Elf64_Shdr.sh_entsize offset")?,
                    "Elf64_Shdr.sh_entsize",
                )?,
            });
        }
        Ok(rows)
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct DecodedElfDynamicRow {
        pub(crate) tag: i64,
        pub(crate) value: u64,
    }

    pub(crate) fn decode_dynamic_rows(
        bytes: &[u8],
        expected_count: usize,
    ) -> Result<Vec<DecodedElfDynamicRow>, Diagnostic> {
        let expected_size = checked_product(
            expected_count,
            ELF64_DYNAMIC_ROW_SIZE,
            "decoded Elf64_Dyn payload size",
        )?;
        require(
            bytes.len() == expected_size,
            "Elf64_Dyn payload has a truncated row or trailing bytes",
        )?;
        let mut rows = Vec::with_capacity(expected_count);
        for ordinal in 0..expected_count {
            let offset = checked_product(ordinal, ELF64_DYNAMIC_ROW_SIZE, "decoded Elf64_Dyn row")?;
            rows.push(DecodedElfDynamicRow {
                tag: read_i64(bytes, offset, "Elf64_Dyn.d_tag")?,
                value: read_u64(
                    bytes,
                    checked_sum(offset, ELF64_DYNAMIC_VALUE_OFFSET, "Elf64_Dyn.d_un offset")?,
                    "Elf64_Dyn.d_un",
                )?,
            });
        }
        Ok(rows)
    }

    pub(crate) struct CandidateValidationError<C> {
        pub(crate) candidate: C,
        pub(crate) diagnostic: Diagnostic,
    }

    macro_rules! seal_stage {
        (
            field: $field:ident,
            contents: $contents:ident,
            validated: $validated:ident,
            validate: $validate:ident,
            fingerprint: $fingerprint:ident,
            message: $message:literal
            $(, vis: $vis:vis)? $(,)?
        ) => {
            $( $vis )? fn validate_candidate(
                candidate: Candidate,
            ) -> Result<$validated, CandidateValidationError<Candidate>> {
                checked::validate_candidate(
                    candidate,
                    |candidate| $validate(&candidate.$field, &candidate.$contents),
                    |candidate| {
                        candidate.$fingerprint == $fingerprint(&candidate.$field, &candidate.$contents)
                    },
                    $message,
                    |candidate| $validated {
                        $field: candidate.$field,
                        $contents: candidate.$contents,
                        $fingerprint: candidate.$fingerprint,
                    },
                )
            }
        };
        (
            field: $field:ident,
            contents: $contents:ident,
            validated: $validated:ident,
            validate: $validate:ident,
            fingerprint: $fingerprint:ident,
            nonzero,
            message: $message:literal
            $(, vis: $vis:vis)? $(,)?
        ) => {
            $( $vis )? fn validate_candidate(
                candidate: Candidate,
            ) -> Result<$validated, CandidateValidationError<Candidate>> {
                checked::validate_candidate(
                    candidate,
                    |candidate| $validate(&candidate.$field, &candidate.$contents),
                    |candidate| {
                        candidate.$fingerprint != 0
                            && candidate.$fingerprint
                                == $fingerprint(&candidate.$field, &candidate.$contents)
                    },
                    $message,
                    |candidate| $validated {
                        $field: candidate.$field,
                        $contents: candidate.$contents,
                        $fingerprint: candidate.$fingerprint,
                    },
                )
            }
        };
        (
            field: $field:ident,
            contents: $contents:ident,
            validated: $validated:ident,
            validate: $validate:ident
            $(, vis: $vis:vis)? $(,)?
        ) => {
            $( $vis )? fn validate_candidate(
                candidate: Candidate,
            ) -> Result<$validated, CandidateValidationError<Candidate>> {
                checked::validate_candidate(
                    candidate,
                    |candidate| $validate(&candidate.$field, &candidate.$contents),
                    |_| true,
                    "",
                    |candidate| $validated {
                        $field: candidate.$field,
                        $contents: candidate.$contents,
                    },
                )
            }
        };
    }
    pub(crate) use seal_stage;

    pub(crate) fn validate_candidate<C, T>(
        candidate: C,
        valid: impl Fn(&C) -> Result<(), Diagnostic>,
        replays: impl Fn(&C) -> bool,
        replay_diagnostic: &'static str,
        validated: impl Fn(C) -> T,
    ) -> Result<T, CandidateValidationError<C>> {
        if let Err(diagnostic) = valid(&candidate) {
            return Err(CandidateValidationError {
                candidate,
                diagnostic,
            });
        }
        if !replays(&candidate) {
            return Err(CandidateValidationError {
                candidate,
                diagnostic: Diagnostic::error(replay_diagnostic),
            });
        }
        Ok(validated(candidate))
    }

    macro_rules! plan_stage {
        (
            plan: $plan:ident,
            derive: $derive:expr,
            contents: $contents:ident,
            fingerprint: $fingerprint:ident,
            error: $error:ident $(,)?
        ) => {
            checked::plan_candidate(
                $plan,
                $derive,
                |$plan, $contents| $fingerprint(&$plan, &$contents),
                |$plan, $contents, fingerprint| Candidate {
                    $plan,
                    $contents,
                    $fingerprint: fingerprint,
                },
                validate_candidate,
                |candidate| candidate.$plan,
                |$plan, diagnostic| Box::new($error { $plan, diagnostic }),
            )
        };
        (
            plan: $plan:ident,
            derive: $derive:expr,
            contents: $contents:ident,
            error: $error:ident $(,)?
        ) => {
            checked::plan_candidate(
                $plan,
                $derive,
                |_, _| 0,
                |$plan, $contents, _fingerprint| Candidate { $plan, $contents },
                validate_candidate,
                |candidate| candidate.$plan,
                |$plan, diagnostic| Box::new($error { $plan, diagnostic }),
            )
        };
    }
    pub(crate) use plan_stage;

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn plan_candidate<P, C, D, V, E>(
        plan: P,
        derive_contents: impl Fn(&P) -> Result<D, Diagnostic>,
        fingerprint: impl Fn(&P, &D) -> u64,
        into_candidate: impl FnOnce(P, D, u64) -> C,
        validate: impl FnOnce(C) -> Result<V, CandidateValidationError<C>>,
        into_plan: impl FnOnce(C) -> P,
        error: impl Fn(P, Diagnostic) -> E,
    ) -> Result<V, E> {
        let contents = match derive_contents(&plan) {
            Ok(contents) => contents,
            Err(diagnostic) => return Err(error(plan, diagnostic)),
        };
        let fingerprint = fingerprint(&plan, &contents);
        let candidate = into_candidate(plan, contents, fingerprint);
        match validate(candidate) {
            Ok(validated) => Ok(validated),
            Err(candidate_error) => Err(error(
                into_plan(candidate_error.candidate),
                candidate_error.diagnostic,
            )),
        }
    }
}

pub(crate) use crate::dynamic_table;
pub(crate) use crate::file_assembly;
pub(crate) use crate::import_sections;
pub(crate) use crate::load_placement;
pub(crate) use crate::procedure_linkage;
pub(crate) use crate::section_headers;

use diagnostics::Diagnostic;
use image::FinalImage;
use target::NormalizedElfInterpreterPlan;

use dynamic_table::dynamic_table_descriptor::{
    ElfDynamicTableSectionDescriptorPlanningError, plan_elf_dynamic_table_section_descriptor,
};
use dynamic_table::dynamic_tag_bytes::{
    ElfDynamicTableSerializationError, serialize_elf_dynamic_table,
};
use dynamic_table::dynamic_tags::{ElfDynamicTagPlanningError, plan_elf_dynamic_tags};
use file_assembly::assembled_dynamic_file::{
    ElfDynamicExecutableAdmissionError, ElfDynamicFileAssemblyError, ValidatedElfDynamicExecutable,
    admit_elf_dynamic_executable, assemble_elf_dynamic_file,
};
use file_assembly::dynamic_file_envelope::{
    ElfDynamicFileEnvelopeSerializationError, serialize_elf_dynamic_file_envelope,
};
use file_assembly::resolved_procedure_linkage::{
    ElfProcedureLinkageApplicationError, apply_elf_procedure_linkage_fixups,
};
use import_sections::dynamic_link::{
    ElfDynamicLinkInputPlanningError, plan_elf_dynamic_link_inputs,
};
use import_sections::dynamic_section_bytes::{
    ElfDynamicSectionSerializationError, serialize_elf_dynamic_sections,
};
use import_sections::dynamic_section_descriptors::{
    ElfDynamicSectionDescriptorPlanningError, plan_elf_dynamic_section_descriptors,
};
use import_sections::dynamic_sections::{
    ElfDynamicSectionPlanningError, plan_elf_dynamic_sections,
};
use load_placement::load_layout::{ElfDynamicLoadLayoutError, plan_elf_dynamic_load_layout};
use load_placement::placed_section_headers::{
    ElfSectionHeaderPlacementApplicationError, apply_elf_section_header_placements,
};
use load_placement::resolved_dynamic_table::{
    ElfDynamicAddressApplicationError, apply_elf_dynamic_address_fixups,
};
use procedure_linkage::dynamic_import_relocations::{
    ElfProcedureLinkageRelocationPlanningError, plan_elf_procedure_linkage_relocations,
};
use procedure_linkage::dynamic_linkage_descriptors::{
    ElfProcedureLinkageSectionDescriptorPlanningError,
    plan_elf_procedure_linkage_section_descriptors,
};
use procedure_linkage::dynamic_linkage_templates::{
    ElfProcedureLinkageTemplatePlanningError, plan_elf_procedure_linkage_templates,
};
use section_headers::relative_section_layout::{
    ElfRelativeSectionPayloadLayoutError, plan_elf_relative_section_payload_layout,
};
use section_headers::section_header_bytes::{
    ElfSectionHeaderTableSerializationError, serialize_elf_section_header_table,
};
use section_headers::section_name_table::{
    ElfSectionNameTablePlanningError, plan_elf_section_name_table,
};
use section_headers::section_payload_roster::{
    ElfIndexedSectionPayloadPlanningError, plan_elf_indexed_section_payloads,
};
use section_headers::section_roster::{
    ElfDynamicSectionRosterPlanningError, plan_elf_dynamic_section_roster,
};

#[derive(Debug)]
#[must_use = "dynamic ELF emission failure retains the exact failing owner"]
pub enum ElfDynamicExecutableEmissionError {
    LinkInputs(Box<ElfDynamicLinkInputPlanningError>),
    DynamicSections(Box<ElfDynamicSectionPlanningError>),
    DynamicSectionBytes(Box<ElfDynamicSectionSerializationError>),
    DynamicSectionDescriptors(Box<ElfDynamicSectionDescriptorPlanningError>),
    ProcedureLinkageRelocations(Box<ElfProcedureLinkageRelocationPlanningError>),
    ProcedureLinkageTemplates(Box<ElfProcedureLinkageTemplatePlanningError>),
    ProcedureLinkageDescriptors(Box<ElfProcedureLinkageSectionDescriptorPlanningError>),
    DynamicTags(Box<ElfDynamicTagPlanningError>),
    DynamicTableBytes(Box<ElfDynamicTableSerializationError>),
    DynamicTableDescriptor(Box<ElfDynamicTableSectionDescriptorPlanningError>),
    SectionNames(Box<ElfSectionNameTablePlanningError>),
    SectionRoster(Box<ElfDynamicSectionRosterPlanningError>),
    SectionHeaderBytes(Box<ElfSectionHeaderTableSerializationError>),
    IndexedPayloads(Box<ElfIndexedSectionPayloadPlanningError>),
    RelativeLayout(Box<ElfRelativeSectionPayloadLayoutError>),
    LoadLayout(Box<ElfDynamicLoadLayoutError>),
    PlacedSectionHeaders(Box<ElfSectionHeaderPlacementApplicationError>),
    ResolvedDynamicTable(Box<ElfDynamicAddressApplicationError>),
    FileEnvelope(Box<ElfDynamicFileEnvelopeSerializationError>),
    ProcedureLinkageApplication(Box<ElfProcedureLinkageApplicationError>),
    FileAssembly(Box<ElfDynamicFileAssemblyError>),
    FinalByteAdmission(Box<ElfDynamicExecutableAdmissionError>),
}

impl ElfDynamicExecutableEmissionError {
    const fn failing_stage(&self) -> (&'static str, &Diagnostic) {
        match self {
            Self::LinkInputs(error) => ("link-inputs", error.diagnostic()),
            Self::DynamicSections(error) => ("dynamic-sections", error.diagnostic()),
            Self::DynamicSectionBytes(error) => ("dynamic-section-bytes", error.diagnostic()),
            Self::DynamicSectionDescriptors(error) => {
                ("dynamic-section-descriptors", error.diagnostic())
            }
            Self::ProcedureLinkageRelocations(error) => {
                ("procedure-linkage-relocations", error.diagnostic())
            }
            Self::ProcedureLinkageTemplates(error) => {
                ("procedure-linkage-templates", error.diagnostic())
            }
            Self::ProcedureLinkageDescriptors(error) => {
                ("procedure-linkage-descriptors", error.diagnostic())
            }
            Self::DynamicTags(error) => ("dynamic-tags", error.diagnostic()),
            Self::DynamicTableBytes(error) => ("dynamic-table-bytes", error.diagnostic()),
            Self::DynamicTableDescriptor(error) => ("dynamic-table-descriptor", error.diagnostic()),
            Self::SectionNames(error) => ("section-names", error.diagnostic()),
            Self::SectionRoster(error) => ("section-roster", error.diagnostic()),
            Self::SectionHeaderBytes(error) => ("section-header-bytes", error.diagnostic()),
            Self::IndexedPayloads(error) => ("indexed-payloads", error.diagnostic()),
            Self::RelativeLayout(error) => ("relative-layout", error.diagnostic()),
            Self::LoadLayout(error) => ("load-layout", error.diagnostic()),
            Self::PlacedSectionHeaders(error) => ("placed-section-headers", error.diagnostic()),
            Self::ResolvedDynamicTable(error) => ("resolved-dynamic-table", error.diagnostic()),
            Self::FileEnvelope(error) => ("file-envelope", error.diagnostic()),
            Self::ProcedureLinkageApplication(error) => {
                ("procedure-linkage-application", error.diagnostic())
            }
            Self::FileAssembly(error) => ("file-assembly", error.diagnostic()),
            Self::FinalByteAdmission(error) => ("final-byte-admission", error.diagnostic()),
        }
    }

    pub const fn stage(&self) -> &'static str {
        self.failing_stage().0
    }

    pub const fn diagnostic(&self) -> &Diagnostic {
        self.failing_stage().1
    }
}

impl std::fmt::Display for ElfDynamicExecutableEmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "dynamic ELF {}: {}",
            self.stage(),
            self.diagnostic()
        )
    }
}

impl std::error::Error for ElfDynamicExecutableEmissionError {}

pub fn emit_elf_dynamic_executable(
    image: FinalImage,
    interpreter: NormalizedElfInterpreterPlan,
) -> Result<ValidatedElfDynamicExecutable, Box<ElfDynamicExecutableEmissionError>> {
    use ElfDynamicExecutableEmissionError as Stage;
    let inputs = plan_elf_dynamic_link_inputs(image, interpreter)
        .map_err(|error| Box::new(Stage::LinkInputs(error)))?;
    let sections = plan_elf_dynamic_sections(inputs)
        .map_err(|error| Box::new(Stage::DynamicSections(error)))?;
    let payloads = serialize_elf_dynamic_sections(sections)
        .map_err(|error| Box::new(Stage::DynamicSectionBytes(error)))?;
    let descriptors = plan_elf_dynamic_section_descriptors(payloads)
        .map_err(|error| Box::new(Stage::DynamicSectionDescriptors(error)))?;
    let linkage = plan_elf_procedure_linkage_relocations(descriptors)
        .map_err(|error| Box::new(Stage::ProcedureLinkageRelocations(error)))?;
    let templates = plan_elf_procedure_linkage_templates(linkage)
        .map_err(|error| Box::new(Stage::ProcedureLinkageTemplates(error)))?;
    let descriptors = plan_elf_procedure_linkage_section_descriptors(templates)
        .map_err(|error| Box::new(Stage::ProcedureLinkageDescriptors(error)))?;
    let tags =
        plan_elf_dynamic_tags(descriptors).map_err(|error| Box::new(Stage::DynamicTags(error)))?;
    let dynamic = serialize_elf_dynamic_table(tags)
        .map_err(|error| Box::new(Stage::DynamicTableBytes(error)))?;
    let descriptor = plan_elf_dynamic_table_section_descriptor(dynamic)
        .map_err(|error| Box::new(Stage::DynamicTableDescriptor(error)))?;
    let names = plan_elf_section_name_table(descriptor)
        .map_err(|error| Box::new(Stage::SectionNames(error)))?;
    let roster = plan_elf_dynamic_section_roster(names)
        .map_err(|error| Box::new(Stage::SectionRoster(error)))?;
    let headers = serialize_elf_section_header_table(roster)
        .map_err(|error| Box::new(Stage::SectionHeaderBytes(error)))?;
    let payloads = plan_elf_indexed_section_payloads(headers)
        .map_err(|error| Box::new(Stage::IndexedPayloads(error)))?;
    let relative = plan_elf_relative_section_payload_layout(payloads)
        .map_err(|error| Box::new(Stage::RelativeLayout(error)))?;
    let load = plan_elf_dynamic_load_layout(relative)
        .map_err(|error| Box::new(Stage::LoadLayout(error)))?;
    let placed = apply_elf_section_header_placements(load)
        .map_err(|error| Box::new(Stage::PlacedSectionHeaders(error)))?;
    let resolved = apply_elf_dynamic_address_fixups(placed)
        .map_err(|error| Box::new(Stage::ResolvedDynamicTable(error)))?;
    let envelope = serialize_elf_dynamic_file_envelope(resolved)
        .map_err(|error| Box::new(Stage::FileEnvelope(error)))?;
    let linkage = apply_elf_procedure_linkage_fixups(envelope)
        .map_err(|error| Box::new(Stage::ProcedureLinkageApplication(error)))?;
    let assembled =
        assemble_elf_dynamic_file(linkage).map_err(|error| Box::new(Stage::FileAssembly(error)))?;
    admit_elf_dynamic_executable(assembled)
        .map_err(|error| Box::new(Stage::FinalByteAdmission(error)))
}

#[cfg(test)]
pub(crate) fn interpreter_path(target: target::TargetProfile) -> &'static [u8] {
    match target {
        target::TargetProfile::LinuxX64 => b"/lib64/ld-linux-\xfc-x86-64.so.2",
        target::TargetProfile::LinuxArm64 => b"/lib/ld-linux-\xfb-aarch64.so.1",
        _ => unreachable!("descriptor fixture uses a Linux target"),
    }
}

pub(crate) const fn image_section_tag(section: image::FinalImageSection) -> u8 {
    match section {
        image::FinalImageSection::Text => 1,
        image::FinalImageSection::Data => 2,
        image::FinalImageSection::Bss => 3,
        image::FinalImageSection::None => 4,
    }
}

pub(crate) fn checked_u64(value: usize, context: &str) -> Result<u64, Diagnostic> {
    u64::try_from(value).map_err(|_| Diagnostic::error(format!("{context} exceeds Elf64_Xword")))
}

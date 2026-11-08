/*
We execute stages 8 through 10 of the dynamic ELF pipeline: planning semantic
`.dynamic` tags (`plan_elf_dynamic_tags`), serializing the `Elf64_Dyn` byte
table (`serialize_elf_dynamic_table`), and planning the `.dynamic` section
descriptor (`plan_elf_dynamic_table_section_descriptor`).
*/

pub(crate) mod dynamic_table_descriptor {
    use super::super::checked::{self, CandidateValidationError};
    use crate::bytes::require_name;
    use crate::dynamic_executable::checked::{checked_product, checked_sum, checked_u32, require};
    use crate::dynamic_executable::dynamic_table::dynamic_tag_bytes::ValidatedElfDynamicTablePayload;
    use crate::dynamic_executable::import_sections::dynamic_section_descriptors::ElfDynamicSectionKind;
    use diagnostics::Diagnostic;

    use crate::constants::{SHF_ALLOC, SHF_WRITE};
    const SHT_DYNAMIC: u32 = 6;
    const ELF64_DYNAMIC_ALIGNMENT: u64 = 8;
    const ELF64_DYNAMIC_ENTRY_SIZE: u64 = 16;
    const UPSTREAM_DESCRIPTOR_COUNT: usize = 11;
    const UPSTREAM_NAME_SEED_SIZE: usize = 113;
    const DYNAMIC_NAME_OFFSET: u32 = 113;
    const DYNAMIC_NAME_SUFFIX: &[u8] = b".dynamic\0";
    #[derive(Debug)]
    #[must_use = "validated ELF dynamic-table descriptor retains the exact payload"]
    pub struct ValidatedElfDynamicTableSectionDescriptorPlan {
        payload: ValidatedElfDynamicTablePayload,
        contents: ElfDynamicTableSectionDescriptorContents,
        non_authoritative_descriptor_compatibility_fingerprint: u64,
    }

    impl ValidatedElfDynamicTableSectionDescriptorPlan {
        pub const fn payload(&self) -> &ValidatedElfDynamicTablePayload {
            &self.payload
        }

        pub const fn descriptor_count(&self) -> usize {
            UPSTREAM_DESCRIPTOR_COUNT + 1
        }

        pub const fn appended_descriptor_count(&self) -> usize {
            1
        }

        pub fn section_name_seed_byte_count(&self) -> usize {
            self.contents.section_name_table_seed.len()
        }

        pub const fn non_authoritative_descriptor_compatibility_fingerprint(&self) -> u64 {
            self.non_authoritative_descriptor_compatibility_fingerprint
        }

        pub(crate) const fn contents(&self) -> &ElfDynamicTableSectionDescriptorContents {
            &self.contents
        }

        pub(crate) fn into_parts(
            self,
        ) -> (
            ValidatedElfDynamicTablePayload,
            ElfDynamicTableSectionDescriptorContents,
        ) {
            (self.payload, self.contents)
        }
    }

    #[derive(Debug)]
    #[must_use = "ELF dynamic-table descriptor rejection retains the serialized payload"]
    pub struct ElfDynamicTableSectionDescriptorPlanningError {
        payload: ValidatedElfDynamicTablePayload,
        diagnostic: Diagnostic,
    }

    image::rejection_carrier!(
        ElfDynamicTableSectionDescriptorPlanningError;
        payload: ValidatedElfDynamicTablePayload,
    );

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ElfDynamicTableSectionDescriptorContents {
        pub(crate) section_name_table_seed: Vec<u8>,
        pub(crate) descriptor: ElfDynamicTableSectionDescriptor,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    pub(crate) enum ElfDynamicTableSectionKind {
        DynamicTable = 1,
    }

    impl ElfDynamicTableSectionKind {
        const fn name(self) -> &'static [u8] {
            match self {
                Self::DynamicTable => b".dynamic",
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ElfDynamicTableSectionDescriptor {
        pub(crate) kind: ElfDynamicTableSectionKind,
        pub(crate) name_offset: u32,
        pub(crate) section_type: u32,
        pub(crate) flags: u64,
        pub(crate) payload_size: u64,
        pub(crate) alignment: u64,
        pub(crate) entry_size: u64,
        pub(crate) link: Option<ElfDynamicSectionKind>,
        pub(crate) info: Option<ElfDynamicSectionKind>,
    }

    struct Candidate {
        payload: ValidatedElfDynamicTablePayload,
        contents: ElfDynamicTableSectionDescriptorContents,
        non_authoritative_descriptor_compatibility_fingerprint: u64,
    }

    pub(crate) fn plan_elf_dynamic_table_section_descriptor(
        payload: ValidatedElfDynamicTablePayload,
    ) -> Result<
        ValidatedElfDynamicTableSectionDescriptorPlan,
        Box<ElfDynamicTableSectionDescriptorPlanningError>,
    > {
        checked::plan_stage!(
            plan: payload,
            derive: derive_contents,
            contents: contents,
            fingerprint: non_authoritative_descriptor_compatibility_fingerprint,
            error: ElfDynamicTableSectionDescriptorPlanningError,
        )
    }

    fn derive_contents(
        payload: &ValidatedElfDynamicTablePayload,
    ) -> Result<ElfDynamicTableSectionDescriptorContents, Diagnostic> {
        let base_seed = base_name_seed(payload);
        let name_offset = checked_u32(base_seed.len(), "dynamic-table name offset")?;
        let mut section_name_table_seed = Vec::with_capacity(checked_sum(
            base_seed.len(),
            DYNAMIC_NAME_SUFFIX.len(),
            "dynamic-table name-seed size",
        )?);
        section_name_table_seed.extend_from_slice(base_seed);
        section_name_table_seed.extend_from_slice(DYNAMIC_NAME_SUFFIX);
        let payload_size = u64::try_from(payload.byte_count())
            .map_err(|_| Diagnostic::error("dynamic-table payload size exceeds Elf64_Xword"))?;
        Ok(ElfDynamicTableSectionDescriptorContents {
            section_name_table_seed,
            descriptor: ElfDynamicTableSectionDescriptor {
                kind: ElfDynamicTableSectionKind::DynamicTable,
                name_offset,
                section_type: SHT_DYNAMIC,
                flags: SHF_WRITE | SHF_ALLOC,
                payload_size,
                alignment: ELF64_DYNAMIC_ALIGNMENT,
                entry_size: ELF64_DYNAMIC_ENTRY_SIZE,
                link: Some(ElfDynamicSectionKind::DynamicString),
                info: None,
            },
        })
    }

    fn base_name_seed(payload: &ValidatedElfDynamicTablePayload) -> &[u8] {
        &payload
            .plan()
            .descriptors()
            .contents()
            .section_name_table_seed
    }

    checked::seal_stage! {
        field: payload,
        contents: contents,
        validated: ValidatedElfDynamicTableSectionDescriptorPlan,
        validate: validate_contents,
        fingerprint: non_authoritative_descriptor_compatibility_fingerprint,
        message: "ELF dynamic-table descriptor compatibility fingerprint does not replay",
    }

    fn validate_contents(
        payload: &ValidatedElfDynamicTablePayload,
        contents: &ElfDynamicTableSectionDescriptorContents,
    ) -> Result<(), Diagnostic> {
        let descriptors = payload.plan().descriptors();
        require(
            descriptors.descriptor_count() == UPSTREAM_DESCRIPTOR_COUNT,
            "dynamic-table descriptor requires the exact sealed eleven-row base",
        )?;
        let base_seed = base_name_seed(payload);
        require(
            base_seed.len() == UPSTREAM_NAME_SEED_SIZE,
            "dynamic-table descriptor requires the exact 113-byte upstream name seed",
        )?;
        let expected_seed_len = checked_sum(
            base_seed.len(),
            DYNAMIC_NAME_SUFFIX.len(),
            "validated dynamic-table name-seed size",
        )?;
        require(
            contents.section_name_table_seed.len() == expected_seed_len
                && contents.section_name_table_seed.get(..base_seed.len()) == Some(base_seed)
                && contents.section_name_table_seed.get(base_seed.len()..)
                    == Some(DYNAMIC_NAME_SUFFIX),
            "dynamic-table section-name seed is not an exact append-only extension",
        )?;

        let dynamic_string_count = descriptors
            .templates()
            .linkage()
            .descriptors()
            .contents()
            .descriptors
            .iter()
            .filter(|row| row.kind == ElfDynamicSectionKind::DynamicString)
            .count();
        require(
            dynamic_string_count == 1,
            "dynamic-table descriptor requires exactly one semantic .dynstr target",
        )?;

        let expected_payload_size = checked_product(
            payload.row_count(),
            usize::try_from(ELF64_DYNAMIC_ENTRY_SIZE)
                .map_err(|_| Diagnostic::error("Elf64_Dyn entry size exceeds usize"))?,
            "validated dynamic-table payload size",
        )?;
        require(
            payload.byte_count() == expected_payload_size,
            "dynamic-table payload size is not an exact Elf64_Dyn row multiple",
        )?;

        let row = &contents.descriptor;
        validate_name(&contents.section_name_table_seed, row)?;
        require(
            row.kind == ElfDynamicTableSectionKind::DynamicTable
                && row.name_offset == DYNAMIC_NAME_OFFSET
                && row.section_type == SHT_DYNAMIC
                && row.flags == SHF_WRITE | SHF_ALLOC
                && row.payload_size
                    == u64::try_from(payload.byte_count()).map_err(|_| {
                        Diagnostic::error("validated dynamic-table payload exceeds Elf64_Xword")
                    })?
                && row.alignment == ELF64_DYNAMIC_ALIGNMENT
                && row.entry_size == ELF64_DYNAMIC_ENTRY_SIZE
                && row.link == Some(ElfDynamicSectionKind::DynamicString)
                && row.info.is_none(),
            "dynamic-table descriptor metadata drifted from its serialized payload",
        )
    }

    fn validate_name(
        seed: &[u8],
        row: &ElfDynamicTableSectionDescriptor,
    ) -> Result<(), Diagnostic> {
        require_name(
            seed,
            row.name_offset,
            row.kind.name(),
            "dynamic-table sh_name",
        )
    }

    fn non_authoritative_descriptor_compatibility_fingerprint(
        payload: &ValidatedElfDynamicTablePayload,
        contents: &ElfDynamicTableSectionDescriptorContents,
    ) -> u64 {
        let mut hash = checked::stage_fingerprint(
            b"omega.elf-dynamic-table-section-descriptor.v1",
            payload.non_authoritative_payload_compatibility_fingerprint(),
        );
        hash.bytes(&contents.section_name_table_seed);
        let row = &contents.descriptor;
        hash.byte(row.kind as u8);
        hash.bytes(&row.name_offset.to_le_bytes());
        hash.bytes(&row.section_type.to_le_bytes());
        hash.bytes(&row.flags.to_le_bytes());
        hash.bytes(&row.payload_size.to_le_bytes());
        hash.bytes(&row.alignment.to_le_bytes());
        hash.bytes(&row.entry_size.to_le_bytes());
        for kind in [row.link, row.info] {
            match kind {
                Some(kind) => {
                    hash.byte(1);
                    hash.byte(kind as u8);
                }
                None => hash.byte(0),
            }
        }
        hash.finish()
    }

    #[cfg(test)]
    mod tests {
        use super::{
            Candidate, DYNAMIC_NAME_SUFFIX, ElfDynamicSectionKind,
            ElfDynamicTableSectionDescriptor, ElfDynamicTableSectionKind, SHF_ALLOC, SHF_WRITE,
            SHT_DYNAMIC, UPSTREAM_NAME_SEED_SIZE, ValidatedElfDynamicTablePayload, base_name_seed,
            checked_product, checked_sum, checked_u32, derive_contents,
            non_authoritative_descriptor_compatibility_fingerprint,
            plan_elf_dynamic_table_section_descriptor, validate_candidate, validate_contents,
            validate_name,
        };
        use crate::serialize_elf_dynamic_table;
        use target::TargetProfile;

        const IMPORTS: [crate::test_support::DynamicImportFixture; 2] =
            crate::test_support::PAIR_IMPORTS;

        fn payload(
            target: TargetProfile,
            imports: &[crate::test_support::DynamicImportFixture],
        ) -> ValidatedElfDynamicTablePayload {
            let tags =
                crate::test_support::dynamic_tag_plan(crate::test_support::dynamic_import_inputs(
                    target,
                    imports,
                    "__omega_dynamic_descriptor_import_",
                    "valid dynamic-descriptor locator",
                    "valid dynamic-descriptor interpreter",
                ));
            serialize_elf_dynamic_table(tags).expect("valid dynamic payload")
        }

        fn candidate(target: TargetProfile) -> Candidate {
            let payload = payload(target, &IMPORTS);
            let contents = derive_contents(&payload).expect("derived dynamic-table descriptor");
            let non_authoritative_descriptor_compatibility_fingerprint =
                non_authoritative_descriptor_compatibility_fingerprint(&payload, &contents);
            Candidate {
                payload,
                contents,
                non_authoritative_descriptor_compatibility_fingerprint,
            }
        }

        #[test]
        fn both_targets_append_exact_name_and_address_free_dynamic_metadata() {
            for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
                let plan = plan_elf_dynamic_table_section_descriptor(payload(target, &IMPORTS))
                    .expect("validated dynamic-table descriptor");
                assert_eq!(plan.payload.plan().descriptors().descriptor_count(), 11);
                assert_eq!(plan.descriptor_count(), 12);
                assert_eq!(plan.appended_descriptor_count(), 1);
                assert_eq!(plan.section_name_seed_byte_count(), 122);
                assert_eq!(
                    &plan.contents.section_name_table_seed[..UPSTREAM_NAME_SEED_SIZE],
                    base_name_seed(plan.payload()),
                );
                let dynstr = &plan
                    .payload()
                    .plan()
                    .descriptors()
                    .templates()
                    .linkage()
                    .descriptors()
                    .payloads()
                    .payloads()
                    .dynstr;
                assert!(dynstr.windows(8).any(|window| window == b"liba\xff.so"));
                assert_eq!(
                    &plan.contents.section_name_table_seed[113..],
                    DYNAMIC_NAME_SUFFIX,
                );
                assert_eq!(
                    plan.contents.descriptor,
                    ElfDynamicTableSectionDescriptor {
                        kind: ElfDynamicTableSectionKind::DynamicTable,
                        name_offset: 113,
                        section_type: SHT_DYNAMIC,
                        flags: SHF_WRITE | SHF_ALLOC,
                        payload_size: 320,
                        alignment: 8,
                        entry_size: 16,
                        link: Some(ElfDynamicSectionKind::DynamicString),
                        info: None,
                    },
                );
                assert_ne!(
                    plan.non_authoritative_descriptor_compatibility_fingerprint(),
                    0
                );
                validate_contents(plan.payload(), &plan.contents)
                    .expect("independent dynamic-table descriptor replay");
            }
        }

        #[test]
        fn import_permutation_preserves_descriptor_identity_and_target_is_bound() {
            let forward = plan_elf_dynamic_table_section_descriptor(payload(
                TargetProfile::LinuxX64,
                &IMPORTS,
            ))
            .unwrap();
            let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
            let reverse = plan_elf_dynamic_table_section_descriptor(payload(
                TargetProfile::LinuxX64,
                &reverse_imports,
            ))
            .unwrap();
            let arm = plan_elf_dynamic_table_section_descriptor(payload(
                TargetProfile::LinuxArm64,
                &IMPORTS,
            ))
            .unwrap();
            assert_eq!(forward.contents, reverse.contents);
            assert_eq!(
                forward.non_authoritative_descriptor_compatibility_fingerprint(),
                reverse.non_authoritative_descriptor_compatibility_fingerprint()
            );
            assert_ne!(
                forward.non_authoritative_descriptor_compatibility_fingerprint(),
                arm.non_authoritative_descriptor_compatibility_fingerprint()
            );
        }

        #[test]
        fn independent_replay_rejects_every_descriptor_field_link_info_and_identity_corruption() {
            let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
                Box::new(|candidate| candidate.contents.section_name_table_seed[0] ^= 1),
                Box::new(|candidate| {
                    candidate.contents.section_name_table_seed[UPSTREAM_NAME_SEED_SIZE] ^= 1
                }),
                Box::new(|candidate| {
                    *candidate
                        .contents
                        .section_name_table_seed
                        .last_mut()
                        .unwrap() ^= 1
                }),
                Box::new(|candidate| {
                    candidate.contents.section_name_table_seed.pop();
                }),
                Box::new(|candidate| candidate.contents.section_name_table_seed.push(0)),
                Box::new(|candidate| candidate.contents.descriptor.name_offset = u32::MAX),
                Box::new(|candidate| candidate.contents.descriptor.section_type = 1),
                Box::new(|candidate| candidate.contents.descriptor.flags ^= SHF_WRITE),
                Box::new(|candidate| candidate.contents.descriptor.payload_size += 1),
                Box::new(|candidate| candidate.contents.descriptor.alignment = 1),
                Box::new(|candidate| candidate.contents.descriptor.entry_size = 8),
                Box::new(|candidate| candidate.contents.descriptor.link = None),
                Box::new(|candidate| {
                    candidate.contents.descriptor.link = Some(ElfDynamicSectionKind::DynamicSymbol)
                }),
                Box::new(|candidate| {
                    candidate.contents.descriptor.info = Some(ElfDynamicSectionKind::DynamicString)
                }),
                Box::new(|candidate| {
                    candidate.non_authoritative_descriptor_compatibility_fingerprint ^= 1
                }),
            ];
            for corrupt in corruptions {
                let mut candidate = candidate(TargetProfile::LinuxArm64);
                let expected_identity = candidate
                    .payload
                    .non_authoritative_payload_compatibility_fingerprint();
                corrupt(&mut candidate);
                let error = validate_candidate(candidate)
                    .expect_err("corrupt dynamic-table descriptor candidate must reject");
                assert_eq!(
                    error
                        .candidate
                        .payload
                        .non_authoritative_payload_compatibility_fingerprint(),
                    expected_identity
                );
            }
        }

        #[test]
        fn malformed_name_offsets_and_arithmetic_reject_without_panicking() {
            assert!(checked_sum(usize::MAX, 1, "sum").is_err());
            assert!(checked_product(usize::MAX, 16, "product").is_err());
            assert!(checked_u32(usize::MAX, "word").is_err());
            let mut descriptor = ElfDynamicTableSectionDescriptor {
                kind: ElfDynamicTableSectionKind::DynamicTable,
                name_offset: u32::MAX,
                section_type: SHT_DYNAMIC,
                flags: SHF_WRITE | SHF_ALLOC,
                payload_size: 16,
                alignment: 8,
                entry_size: 16,
                link: Some(ElfDynamicSectionKind::DynamicString),
                info: None,
            };
            assert!(validate_name(b"\0.dynamic\0", &descriptor).is_err());
            descriptor.name_offset = 1;
            assert!(validate_name(b"\0.dynamic", &descriptor).is_err());
            descriptor.name_offset = 2;
            assert!(validate_name(b"\0.dynamic\0", &descriptor).is_err());
        }
    }
}

pub(crate) mod dynamic_tag_bytes {
    use super::super::checked::{self, CandidateValidationError};
    use crate::bytes::write_u64;
    use crate::constants::{
        ELF64_DYNAMIC_ROW_SIZE, ELF64_DYNAMIC_VALUE_OFFSET, ELF64_DYNAMIC_VALUE_SIZE,
    };
    use crate::dynamic_executable::checked::{
        checked_product, checked_sum, decode_dynamic_rows, read_u64, require,
    };
    use crate::dynamic_executable::dynamic_table::dynamic_tags::{
        ElfDynamicAddressTarget, ElfDynamicTagContents, ElfDynamicValue,
        ValidatedElfDynamicTagPlan, encoded_value,
    };
    use diagnostics::Diagnostic;

    #[derive(Debug)]
    #[must_use = "validated ELF dynamic bytes retain the exact semantic tag plan"]
    pub struct ValidatedElfDynamicTablePayload {
        plan: ValidatedElfDynamicTagPlan,
        contents: ElfDynamicTablePayloadContents,
        non_authoritative_payload_compatibility_fingerprint: u64,
    }

    impl ValidatedElfDynamicTablePayload {
        pub const fn plan(&self) -> &ValidatedElfDynamicTagPlan {
            &self.plan
        }

        pub fn row_count(&self) -> usize {
            self.plan.row_count()
        }

        pub fn byte_count(&self) -> usize {
            self.contents.bytes.len()
        }
        pub const fn non_authoritative_payload_compatibility_fingerprint(&self) -> u64 {
            self.non_authoritative_payload_compatibility_fingerprint
        }

        pub(crate) const fn contents(&self) -> &ElfDynamicTablePayloadContents {
            &self.contents
        }

        pub(crate) fn into_parts(
            self,
        ) -> (ValidatedElfDynamicTagPlan, ElfDynamicTablePayloadContents) {
            (self.plan, self.contents)
        }
    }

    #[derive(Debug)]
    #[must_use = "ELF dynamic serialization rejection retains the semantic plan"]
    pub struct ElfDynamicTableSerializationError {
        plan: ValidatedElfDynamicTagPlan,
        diagnostic: Diagnostic,
    }

    image::rejection_carrier!(ElfDynamicTableSerializationError; plan: ValidatedElfDynamicTagPlan);

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ElfDynamicTablePayloadContents {
        pub(crate) bytes: Vec<u8>,
        pub(crate) address_fixups: Vec<ElfDynamicPayloadAddressFixup>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    pub(crate) enum ElfDynamicPayloadFixupKind {
        Elf64AbsoluteAddress = 1,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ElfDynamicPayloadAddressFixup {
        pub(crate) row_ordinal: u32,
        pub(crate) byte_offset: usize,
        pub(crate) byte_width: u8,
        pub(crate) kind: ElfDynamicPayloadFixupKind,
        pub(crate) target: ElfDynamicAddressTarget,
    }

    struct Candidate {
        plan: ValidatedElfDynamicTagPlan,
        contents: ElfDynamicTablePayloadContents,
        non_authoritative_payload_compatibility_fingerprint: u64,
    }

    pub(crate) fn serialize_elf_dynamic_table(
        plan: ValidatedElfDynamicTagPlan,
    ) -> Result<ValidatedElfDynamicTablePayload, Box<ElfDynamicTableSerializationError>> {
        checked::plan_stage!(
            plan: plan,
            derive: encode_contents,
            contents: contents,
            fingerprint: non_authoritative_payload_compatibility_fingerprint,
            error: ElfDynamicTableSerializationError,
        )
    }

    fn encode_contents(
        plan: &ValidatedElfDynamicTagPlan,
    ) -> Result<ElfDynamicTablePayloadContents, Diagnostic> {
        let semantic = plan.contents();
        let mut bytes = Vec::with_capacity(checked_product(
            semantic.rows.len(),
            ELF64_DYNAMIC_ROW_SIZE,
            "ELF64 dynamic payload size",
        )?);
        for row in &semantic.rows {
            write_u64(&mut bytes, (row.tag as i64) as u64);
            write_u64(&mut bytes, encoded_value(row.value));
        }
        let address_fixups = semantic
            .address_obligations
            .iter()
            .map(|obligation| {
                let row_offset = checked_product(
                    obligation.row_ordinal as usize,
                    ELF64_DYNAMIC_ROW_SIZE,
                    "ELF64 dynamic fixup row offset",
                )?;
                Ok(ElfDynamicPayloadAddressFixup {
                    row_ordinal: obligation.row_ordinal,
                    byte_offset: checked_sum(
                        row_offset,
                        ELF64_DYNAMIC_VALUE_OFFSET,
                        "ELF64 dynamic fixup value offset",
                    )?,
                    byte_width: obligation.byte_width,
                    kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
                    target: obligation.target,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        Ok(ElfDynamicTablePayloadContents {
            bytes,
            address_fixups,
        })
    }

    checked::seal_stage! {
        field: plan,
        contents: contents,
        validated: ValidatedElfDynamicTablePayload,
        validate: validate_contents,
        fingerprint: non_authoritative_payload_compatibility_fingerprint,
        message: "ELF dynamic payload compatibility fingerprint does not replay",
    }

    fn validate_contents(
        plan: &ValidatedElfDynamicTagPlan,
        contents: &ElfDynamicTablePayloadContents,
    ) -> Result<(), Diagnostic> {
        let semantic = plan.contents();
        let decoded = decode_dynamic_rows(&contents.bytes, semantic.rows.len())?;
        require(
            decoded.len() == semantic.rows.len(),
            "decoded Elf64_Dyn row count drifted from the semantic plan",
        )?;
        for (decoded, semantic) in decoded.iter().zip(&semantic.rows) {
            require(
                decoded.tag == semantic.tag as i64
                    && decoded.value == encoded_value(semantic.value),
                "decoded Elf64_Dyn tag or value drifted from the semantic row",
            )?;
        }
        validate_fixups(&contents.bytes, semantic, &contents.address_fixups)
    }

    fn validate_fixups(
        bytes: &[u8],
        semantic: &ElfDynamicTagContents,
        fixups: &[ElfDynamicPayloadAddressFixup],
    ) -> Result<(), Diagnostic> {
        require(
            fixups.len() == semantic.address_obligations.len(),
            "serialized ELF dynamic address-fixup count is not exact",
        )?;
        for (fixup, obligation) in fixups.iter().zip(&semantic.address_obligations) {
            let row_offset = checked_product(
                obligation.row_ordinal as usize,
                ELF64_DYNAMIC_ROW_SIZE,
                "replayed Elf64_Dyn fixup row",
            )?;
            let expected_offset = checked_sum(
                row_offset,
                ELF64_DYNAMIC_VALUE_OFFSET,
                "replayed Elf64_Dyn fixup value",
            )?;
            require(
                fixup.row_ordinal == obligation.row_ordinal
                    && fixup.byte_offset == expected_offset
                    && fixup.byte_width == ELF64_DYNAMIC_VALUE_SIZE
                    && fixup.byte_width == obligation.byte_width
                    && fixup.kind == ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress
                    && fixup.target == obligation.target,
                "serialized ELF dynamic address fixup drifted from its semantic obligation",
            )?;
            require(
                read_u64(bytes, fixup.byte_offset, "Elf64_Dyn address placeholder")? == 0,
                "serialized Elf64_Dyn address field is not an exact zero placeholder",
            )?;
        }
        for (index, row) in semantic.rows.iter().enumerate() {
            let fixup_count = fixups
                .iter()
                .filter(|fixup| fixup.row_ordinal as usize == index)
                .count();
            require(
                fixup_count
                    == usize::from(matches!(row.value, ElfDynamicValue::AddressPlaceholder)),
                "serialized Elf64_Dyn row has a missing, duplicate, or orphan address fixup",
            )?;
        }
        validate_fixup_coverage(fixups)
    }

    fn validate_fixup_coverage(fixups: &[ElfDynamicPayloadAddressFixup]) -> Result<(), Diagnostic> {
        for (index, fixup) in fixups.iter().enumerate() {
            let end = checked_sum(
                fixup.byte_offset,
                usize::from(fixup.byte_width),
                "Elf64_Dyn fixup end",
            )?;
            for other in &fixups[index + 1..] {
                let other_end = checked_sum(
                    other.byte_offset,
                    usize::from(other.byte_width),
                    "Elf64_Dyn fixup end",
                )?;
                require(
                    end <= other.byte_offset || other_end <= fixup.byte_offset,
                    "serialized Elf64_Dyn address fixups overlap or duplicate one field",
                )?;
            }
        }
        Ok(())
    }

    fn non_authoritative_payload_compatibility_fingerprint(
        plan: &ValidatedElfDynamicTagPlan,
        contents: &ElfDynamicTablePayloadContents,
    ) -> u64 {
        let mut hash = checked::stage_fingerprint(
            b"omega.elf-dynamic-table-payload.v1",
            plan.non_authoritative_tag_compatibility_fingerprint(),
        );
        hash.bytes(&contents.bytes);
        hash.bytes(&(contents.address_fixups.len() as u64).to_le_bytes());
        for fixup in &contents.address_fixups {
            hash.bytes(&fixup.row_ordinal.to_le_bytes());
            hash.bytes(&(fixup.byte_offset as u64).to_le_bytes());
            hash.byte(fixup.byte_width);
            hash.byte(fixup.kind as u8);
            hash.byte(fixup.target as u8);
        }
        hash.finish()
    }

    #[cfg(test)]
    mod tests {
        use super::{
            Candidate, ELF64_DYNAMIC_ROW_SIZE, ELF64_DYNAMIC_VALUE_OFFSET, ElfDynamicAddressTarget,
            ElfDynamicPayloadAddressFixup, ElfDynamicPayloadFixupKind, ElfDynamicValue,
            ValidatedElfDynamicTagPlan, checked_product, checked_sum, decode_dynamic_rows,
            encode_contents, non_authoritative_payload_compatibility_fingerprint, read_u64,
            serialize_elf_dynamic_table, validate_candidate, validate_contents,
            validate_fixup_coverage,
        };
        use crate::bytes::name_at;
        use crate::dynamic_executable::checked::read_i64;
        use target::TargetProfile;

        fn address_fixup(
            row_ordinal: u32,
            byte_offset: usize,
            target: ElfDynamicAddressTarget,
        ) -> ElfDynamicPayloadAddressFixup {
            ElfDynamicPayloadAddressFixup {
                row_ordinal,
                byte_offset,
                byte_width: 8,
                kind: ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
                target,
            }
        }

        const IMPORTS: [crate::test_support::DynamicImportFixture; 2] =
            crate::test_support::PAIR_IMPORTS;

        fn tag_plan(
            target: TargetProfile,
            imports: &[crate::test_support::DynamicImportFixture],
        ) -> ValidatedElfDynamicTagPlan {
            crate::test_support::dynamic_tag_plan(crate::test_support::dynamic_import_inputs(
                target,
                imports,
                "__omega_dynamic_byte_import_",
                "valid dynamic-byte locator",
                "valid dynamic-byte interpreter",
            ))
        }

        fn candidate(target: TargetProfile) -> Candidate {
            let plan = tag_plan(target, &IMPORTS);
            let contents = encode_contents(&plan).expect("encoded dynamic bytes");
            let non_authoritative_payload_compatibility_fingerprint =
                non_authoritative_payload_compatibility_fingerprint(&plan, &contents);
            Candidate {
                plan,
                contents,
                non_authoritative_payload_compatibility_fingerprint,
            }
        }

        fn row(bytes: &[u8], ordinal: usize) -> &[u8] {
            let start = ordinal * ELF64_DYNAMIC_ROW_SIZE;
            &bytes[start..start + ELF64_DYNAMIC_ROW_SIZE]
        }

        #[test]
        fn both_targets_serialize_exact_elf64_lsb_rows_and_fixup_coordinates() {
            for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
                let payload = serialize_elf_dynamic_table(tag_plan(target, &IMPORTS))
                    .expect("validated dynamic payload");
                assert_eq!(payload.row_count(), 20);
                assert_eq!(payload.byte_count(), 320);
                assert_eq!(payload.contents().address_fixups.len(), 9);
                assert_ne!(
                    payload.non_authoritative_payload_compatibility_fingerprint(),
                    0
                );

                let bytes = &payload.contents.bytes;
                let first_needed = match payload.plan.contents().rows[0].value {
                    ElfDynamicValue::NeededStringOffset(offset) => offset,
                    _ => panic!("first row must be DT_NEEDED"),
                };
                assert_eq!(&row(bytes, 0)[..8], &[1, 0, 0, 0, 0, 0, 0, 0]);
                assert_eq!(
                    &row(bytes, 0)[8..],
                    &(u64::from(first_needed)).to_le_bytes(),
                );
                assert_eq!(&row(bytes, 5)[..8], &[0xf5, 0xfe, 0xff, 0x6f, 0, 0, 0, 0]);
                assert_eq!(&row(bytes, 10)[..8], &[20, 0, 0, 0, 0, 0, 0, 0]);
                assert_eq!(&row(bytes, 10)[8..], &[7, 0, 0, 0, 0, 0, 0, 0]);
                assert_eq!(&row(bytes, 12)[..8], &[7, 0, 0, 0, 0, 0, 0, 0]);
                assert_eq!(
                    row(bytes, 13),
                    &[8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
                );
                assert_eq!(
                    row(bytes, 14),
                    &[9, 0, 0, 0, 0, 0, 0, 0, 24, 0, 0, 0, 0, 0, 0, 0]
                );
                assert_eq!(&row(bytes, 15)[..8], &[0xf0, 0xff, 0xff, 0x6f, 0, 0, 0, 0],);
                assert_eq!(&row(bytes, 18)[..8], &30_u64.to_le_bytes());
                assert_eq!(&row(bytes, 18)[8..], &8_u64.to_le_bytes());
                assert_eq!(row(bytes, 19), &[0; 16]);
                assert_eq!(
                    payload.contents.address_fixups,
                    [
                        address_fixup(3, 56, ElfDynamicAddressTarget::ProcedureGot),
                        address_fixup(4, 72, ElfDynamicAddressTarget::SystemVHash),
                        address_fixup(5, 88, ElfDynamicAddressTarget::GnuHash),
                        address_fixup(6, 104, ElfDynamicAddressTarget::DynamicString),
                        address_fixup(7, 120, ElfDynamicAddressTarget::DynamicSymbol),
                        address_fixup(11, 184, ElfDynamicAddressTarget::ProcedureRelocation),
                        address_fixup(12, 200, ElfDynamicAddressTarget::GeneralRelocation),
                        address_fixup(15, 248, ElfDynamicAddressTarget::GnuSymbolVersion),
                        address_fixup(16, 264, ElfDynamicAddressTarget::GnuVersionRequirement),
                    ],
                );
                validate_contents(payload.plan(), &payload.contents)
                    .expect("independent dynamic-byte replay");
            }
        }

        #[test]
        fn encoded_needed_offsets_still_select_exact_raw_non_utf8_names() {
            let payload =
                serialize_elf_dynamic_table(tag_plan(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
            let dynstr = &payload
                .plan
                .descriptors()
                .templates()
                .linkage()
                .descriptors()
                .payloads()
                .payloads()
                .dynstr;
            let names = (0..payload.plan.needed_row_count())
                .map(|ordinal| {
                    let offset = read_u64(
                        &payload.contents.bytes,
                        ordinal * ELF64_DYNAMIC_ROW_SIZE + ELF64_DYNAMIC_VALUE_OFFSET,
                        "DT_NEEDED offset",
                    )
                    .unwrap() as u32;
                    name_at(dynstr, offset, "DT_NEEDED string offset").unwrap()
                })
                .collect::<Vec<_>>();
            assert_eq!(names, [b"liba\xff.so".as_slice(), b"libb.so".as_slice()]);
        }

        #[test]
        fn import_permutation_preserves_bytes_while_target_remains_identity_bound() {
            let forward =
                serialize_elf_dynamic_table(tag_plan(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
            let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
            let reverse =
                serialize_elf_dynamic_table(tag_plan(TargetProfile::LinuxX64, &reverse_imports))
                    .unwrap();
            let arm =
                serialize_elf_dynamic_table(tag_plan(TargetProfile::LinuxArm64, &IMPORTS)).unwrap();
            assert_eq!(forward.contents, reverse.contents);
            assert_eq!(
                forward.non_authoritative_payload_compatibility_fingerprint(),
                reverse.non_authoritative_payload_compatibility_fingerprint()
            );
            assert_eq!(forward.contents.bytes, arm.contents.bytes);
            assert_ne!(
                forward.non_authoritative_payload_compatibility_fingerprint(),
                arm.non_authoritative_payload_compatibility_fingerprint()
            );
        }

        #[test]
        fn independent_decoder_rejects_every_tag_value_placeholder_and_boundary_corruption() {
            let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
                Box::new(|candidate| candidate.contents.bytes[0] ^= 1),
                Box::new(|candidate| candidate.contents.bytes[8] ^= 1),
                Box::new(|candidate| candidate.contents.bytes[2 * 16 + 8] ^= 1),
                Box::new(|candidate| candidate.contents.bytes[3 * 16 + 8] = 1),
                Box::new(|candidate| candidate.contents.bytes[8 * 16 + 8] ^= 1),
                Box::new(|candidate| candidate.contents.bytes[9 * 16 + 8] ^= 1),
                Box::new(|candidate| candidate.contents.bytes[10 * 16 + 8] ^= 1),
                Box::new(|candidate| candidate.contents.bytes[14 * 16 + 8] ^= 1),
                Box::new(|candidate| candidate.contents.bytes[17 * 16 + 8] ^= 1),
                Box::new(|candidate| candidate.contents.bytes[18 * 16] = 1),
                Box::new(|candidate| candidate.contents.bytes[18 * 16 + 8] = 1),
                Box::new(|candidate| candidate.contents.bytes[12 * 16..12 * 16 + 8].reverse()),
                Box::new(|candidate| {
                    candidate.contents.bytes.pop();
                }),
                Box::new(|candidate| candidate.contents.bytes.push(0)),
                Box::new(|candidate| {
                    candidate.non_authoritative_payload_compatibility_fingerprint ^= 1
                }),
            ];
            for corrupt in corruptions {
                let mut candidate = candidate(TargetProfile::LinuxX64);
                let expected_identity = candidate
                    .plan
                    .non_authoritative_tag_compatibility_fingerprint();
                corrupt(&mut candidate);
                let error = validate_candidate(candidate)
                    .expect_err("corrupt Elf64_Dyn payload must reject");
                assert_eq!(
                    error
                        .candidate
                        .plan
                        .non_authoritative_tag_compatibility_fingerprint(),
                    expected_identity,
                    "dynamic-byte rejection retains exact semantic-plan custody",
                );
            }
        }

        #[test]
        fn independent_replay_rejects_every_fixup_coordinate_kind_and_target_corruption() {
            let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
                Box::new(|candidate| {
                    candidate.contents.address_fixups.pop();
                }),
                Box::new(|candidate| {
                    candidate
                        .contents
                        .address_fixups
                        .push(candidate.contents.address_fixups[0])
                }),
                Box::new(|candidate| candidate.contents.address_fixups.swap(0, 1)),
                Box::new(|candidate| candidate.contents.address_fixups[0].row_ordinal = u32::MAX),
                Box::new(|candidate| candidate.contents.address_fixups[0].byte_offset += 1),
                Box::new(|candidate| candidate.contents.address_fixups[0].byte_width = 4),
                Box::new(|candidate| {
                    candidate.contents.address_fixups[0].target =
                        ElfDynamicAddressTarget::DynamicString
                }),
            ];
            for corrupt in corruptions {
                let mut candidate = candidate(TargetProfile::LinuxArm64);
                let expected_identity = candidate
                    .plan
                    .non_authoritative_tag_compatibility_fingerprint();
                corrupt(&mut candidate);
                let error = validate_candidate(candidate)
                    .expect_err("corrupt Elf64_Dyn address fixup must reject");
                assert_eq!(
                    error
                        .candidate
                        .plan
                        .non_authoritative_tag_compatibility_fingerprint(),
                    expected_identity
                );
            }
        }

        #[test]
        fn decoder_and_fixup_checks_reject_overflow_and_malformed_inputs_without_panicking() {
            assert!(checked_product(usize::MAX, 16, "product").is_err());
            assert!(checked_sum(usize::MAX, 8, "sum").is_err());
            assert!(decode_dynamic_rows(&[], usize::MAX).is_err());
            assert!(decode_dynamic_rows(&[0; 15], 1).is_err());
            assert!(decode_dynamic_rows(&[0; 17], 1).is_err());
            assert!(read_i64(&[], usize::MAX, "tag").is_err());
            assert!(read_u64(&[0; 7], 0, "value").is_err());

            let overflow =
                address_fixup(u32::MAX, usize::MAX, ElfDynamicAddressTarget::ProcedureGot);
            assert!(validate_fixup_coverage(&[overflow, overflow]).is_err());
        }
    }
}

pub(crate) mod dynamic_tags {
    use target::compatibility_fingerprint::Fnv1a;

    use super::super::checked::{self, CandidateValidationError};
    use crate::bytes::name_at;
    use crate::constants::ELF64_RELA_SIZE;
    use crate::dynamic_executable::checked::{checked_product, checked_sum, checked_u32, require};
    use crate::dynamic_executable::procedure_linkage::dynamic_linkage_descriptors::{
        ElfProcedureLinkageSectionKind, ValidatedElfProcedureLinkageSectionDescriptorPlan,
    };
    use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::{
        ElfProcedureLinkageFixupKind, ElfProcedureLinkageFixupStorage,
        ElfProcedureLinkageSemanticTarget,
    };
    use diagnostics::Diagnostic;
    use target::TargetProfile;

    const ELF64_DYNAMIC_SYMBOL_SIZE: u64 = 24;
    const ELF64_DYN_VALUE_SIZE: u8 = 8;
    const UNVERSIONED_FIXED_ROW_COUNT: usize = 15;
    const DF_BIND_NOW: u64 = 0x8;
    const UNVERSIONED_ADDRESS_COUNT: usize = 7;
    #[derive(Debug)]
    #[must_use = "validated ELF dynamic tags retain the exact descriptor carrier"]
    pub struct ValidatedElfDynamicTagPlan {
        descriptors: ValidatedElfProcedureLinkageSectionDescriptorPlan,
        contents: ElfDynamicTagContents,
        non_authoritative_tag_compatibility_fingerprint: u64,
    }

    impl ValidatedElfDynamicTagPlan {
        pub const fn descriptors(&self) -> &ValidatedElfProcedureLinkageSectionDescriptorPlan {
            &self.descriptors
        }

        pub(crate) fn has_version_requirements(&self) -> bool {
            !structural_contents(&self.descriptors).verneed.is_empty()
        }

        pub fn row_count(&self) -> usize {
            self.contents.rows.len()
        }

        #[cfg(test)]
        pub(crate) fn needed_row_count(&self) -> usize {
            self.descriptors
                .templates()
                .linkage()
                .descriptors()
                .payloads()
                .plan()
                .needed_object_count()
        }

        pub(crate) fn address_obligation_count(&self) -> usize {
            self.contents.address_obligations.len()
        }

        pub const fn non_authoritative_tag_compatibility_fingerprint(&self) -> u64 {
            self.non_authoritative_tag_compatibility_fingerprint
        }

        pub(crate) const fn contents(&self) -> &ElfDynamicTagContents {
            &self.contents
        }

        pub(crate) fn into_parts(
            self,
        ) -> (
            ValidatedElfProcedureLinkageSectionDescriptorPlan,
            ElfDynamicTagContents,
        ) {
            (self.descriptors, self.contents)
        }
    }

    #[derive(Debug)]
    #[must_use = "ELF dynamic-tag rejection retains the eleven-section carrier"]
    pub struct ElfDynamicTagPlanningError {
        descriptors: ValidatedElfProcedureLinkageSectionDescriptorPlan,
        diagnostic: Diagnostic,
    }

    image::rejection_carrier!(
        ElfDynamicTagPlanningError;
        descriptors: ValidatedElfProcedureLinkageSectionDescriptorPlan,
    );

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ElfDynamicTagContents {
        pub(crate) rows: Vec<ElfDynamicSemanticRow>,
        pub(crate) address_obligations: Vec<ElfDynamicAddressObligation>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(i64)]
    pub(crate) enum ElfDynamicTag {
        Null = 0,
        Needed = 1,
        ProcedureRelocationSize = 2,
        ProcedureGot = 3,
        SystemVHash = 4,
        DynamicString = 5,
        DynamicSymbol = 6,
        Rela = 7,
        GeneralRelocationSize = 8,
        GeneralRelocationEntrySize = 9,
        DynamicStringSize = 10,
        DynamicSymbolEntrySize = 11,
        ProcedureRelocationKind = 20,
        ProcedureRelocation = 23,
        Flags = 30,
        GnuSymbolVersion = 0x6fff_fff0,
        GnuHash = 0x6fff_fef5,
        GnuVersionRequirement = 0x6fff_fffe,
        GnuVersionRequirementCount = 0x6fff_ffff,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum ElfDynamicValue {
        NeededStringOffset(u32),
        ProcedureRelocationByteCount(u64),
        GeneralRelocationByteCount(u64),
        GeneralRelocationEntryByteCount(u64),
        AddressPlaceholder,
        DynamicStringByteCount(u64),
        DynamicSymbolEntryByteCount(u64),
        RelocationTag(ElfDynamicTag),
        VersionRequirementRecordCount(u64),
        BindingFlags(u64),
        Null,
    }

    pub(crate) const fn encoded_value(value: ElfDynamicValue) -> u64 {
        match value {
            ElfDynamicValue::NeededStringOffset(offset) => offset as u64,
            ElfDynamicValue::ProcedureRelocationByteCount(count)
            | ElfDynamicValue::GeneralRelocationByteCount(count)
            | ElfDynamicValue::GeneralRelocationEntryByteCount(count)
            | ElfDynamicValue::DynamicStringByteCount(count)
            | ElfDynamicValue::DynamicSymbolEntryByteCount(count)
            | ElfDynamicValue::VersionRequirementRecordCount(count)
            | ElfDynamicValue::BindingFlags(count) => count,
            ElfDynamicValue::RelocationTag(tag) => (tag as i64) as u64,
            ElfDynamicValue::AddressPlaceholder | ElfDynamicValue::Null => 0,
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ElfDynamicSemanticRow {
        pub(crate) tag: ElfDynamicTag,
        pub(crate) value: ElfDynamicValue,
    }

    pub(crate) type ElfDynamicAddressTarget = crate::ElfDynamicAddressApplicationTarget;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ElfDynamicAddressObligation {
        pub(crate) row_ordinal: u32,
        pub(crate) byte_width: u8,
        pub(crate) target: ElfDynamicAddressTarget,
    }

    struct Candidate {
        descriptors: ValidatedElfProcedureLinkageSectionDescriptorPlan,
        contents: ElfDynamicTagContents,
        non_authoritative_tag_compatibility_fingerprint: u64,
    }

    pub(crate) fn plan_elf_dynamic_tags(
        descriptors: ValidatedElfProcedureLinkageSectionDescriptorPlan,
    ) -> Result<ValidatedElfDynamicTagPlan, Box<ElfDynamicTagPlanningError>> {
        checked::plan_stage!(
            plan: descriptors,
            derive: derive_contents,
            contents: contents,
            fingerprint: non_authoritative_tag_compatibility_fingerprint,
            error: ElfDynamicTagPlanningError,
        )
    }

    fn derive_contents(
        descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
    ) -> Result<ElfDynamicTagContents, Diagnostic> {
        let structural = structural_contents(descriptors);
        let row_capacity = checked_sum(
            structural.needed.len(),
            UNVERSIONED_FIXED_ROW_COUNT + 3 * usize::from(!structural.verneed.is_empty()),
            "ELF dynamic row count",
        )?;
        let mut rows = Vec::with_capacity(row_capacity);
        let mut address_obligations = Vec::with_capacity(
            UNVERSIONED_ADDRESS_COUNT + 2 * usize::from(!structural.verneed.is_empty()),
        );
        for needed in &structural.needed {
            rows.push(ElfDynamicSemanticRow {
                tag: ElfDynamicTag::Needed,
                value: ElfDynamicValue::NeededStringOffset(*needed),
            });
        }
        rows.push(ElfDynamicSemanticRow {
            tag: ElfDynamicTag::ProcedureRelocationSize,
            value: ElfDynamicValue::ProcedureRelocationByteCount(super::super::checked_u64(
                descriptors.templates().procedure_relocation_byte_count(),
                "procedure relocation byte count",
            )?),
        });
        for (tag, target) in [
            (
                ElfDynamicTag::ProcedureGot,
                ElfDynamicAddressTarget::ProcedureGot,
            ),
            (
                ElfDynamicTag::SystemVHash,
                ElfDynamicAddressTarget::SystemVHash,
            ),
            (ElfDynamicTag::GnuHash, ElfDynamicAddressTarget::GnuHash),
            (
                ElfDynamicTag::DynamicString,
                ElfDynamicAddressTarget::DynamicString,
            ),
            (
                ElfDynamicTag::DynamicSymbol,
                ElfDynamicAddressTarget::DynamicSymbol,
            ),
        ] {
            push_address_row(&mut rows, &mut address_obligations, tag, target)?;
        }
        rows.push(ElfDynamicSemanticRow {
            tag: ElfDynamicTag::DynamicStringSize,
            value: ElfDynamicValue::DynamicStringByteCount(super::super::checked_u64(
                dynamic_payloads(descriptors).dynamic_string_byte_count(),
                "dynamic string byte count",
            )?),
        });
        rows.push(ElfDynamicSemanticRow {
            tag: ElfDynamicTag::DynamicSymbolEntrySize,
            value: ElfDynamicValue::DynamicSymbolEntryByteCount(ELF64_DYNAMIC_SYMBOL_SIZE),
        });
        rows.push(ElfDynamicSemanticRow {
            tag: ElfDynamicTag::ProcedureRelocationKind,
            value: ElfDynamicValue::RelocationTag(ElfDynamicTag::Rela),
        });
        push_address_row(
            &mut rows,
            &mut address_obligations,
            ElfDynamicTag::ProcedureRelocation,
            ElfDynamicAddressTarget::ProcedureRelocation,
        )?;
        push_address_row(
            &mut rows,
            &mut address_obligations,
            ElfDynamicTag::Rela,
            ElfDynamicAddressTarget::GeneralRelocation,
        )?;
        rows.push(ElfDynamicSemanticRow {
            tag: ElfDynamicTag::GeneralRelocationSize,
            value: ElfDynamicValue::GeneralRelocationByteCount(super::super::checked_u64(
                descriptors.templates().general_relocation_byte_count(),
                "general relocation byte count",
            )?),
        });
        rows.push(ElfDynamicSemanticRow {
            tag: ElfDynamicTag::GeneralRelocationEntrySize,
            value: ElfDynamicValue::GeneralRelocationEntryByteCount(super::super::checked_u64(
                ELF64_RELA_SIZE,
                "general relocation entry size",
            )?),
        });
        if !structural.verneed.is_empty() {
            push_address_row(
                &mut rows,
                &mut address_obligations,
                ElfDynamicTag::GnuSymbolVersion,
                ElfDynamicAddressTarget::GnuSymbolVersion,
            )?;
            push_address_row(
                &mut rows,
                &mut address_obligations,
                ElfDynamicTag::GnuVersionRequirement,
                ElfDynamicAddressTarget::GnuVersionRequirement,
            )?;
            rows.push(ElfDynamicSemanticRow {
                tag: ElfDynamicTag::GnuVersionRequirementCount,
                value: ElfDynamicValue::VersionRequirementRecordCount(super::super::checked_u64(
                    structural.verneed.len(),
                    "GNU version-requirement record count",
                )?),
            });
        }
        rows.push(ElfDynamicSemanticRow {
            tag: ElfDynamicTag::Flags,
            value: ElfDynamicValue::BindingFlags(DF_BIND_NOW),
        });
        rows.push(ElfDynamicSemanticRow {
            tag: ElfDynamicTag::Null,
            value: ElfDynamicValue::Null,
        });
        Ok(ElfDynamicTagContents {
            rows,
            address_obligations,
        })
    }

    fn push_address_row(
        rows: &mut Vec<ElfDynamicSemanticRow>,
        obligations: &mut Vec<ElfDynamicAddressObligation>,
        tag: ElfDynamicTag,
        target: ElfDynamicAddressTarget,
    ) -> Result<(), Diagnostic> {
        let row_ordinal = checked_u32(rows.len(), "ELF dynamic address-row ordinal")?;
        rows.push(ElfDynamicSemanticRow {
            tag,
            value: ElfDynamicValue::AddressPlaceholder,
        });
        obligations.push(ElfDynamicAddressObligation {
            row_ordinal,
            byte_width: ELF64_DYN_VALUE_SIZE,
            target,
        });
        Ok(())
    }

    fn dynamic_payloads(
        descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
    ) -> &crate::dynamic_executable::import_sections::dynamic_section_bytes::ValidatedElfDynamicSectionPayloads{
        descriptors.templates().linkage().descriptors().payloads()
    }

    fn structural_contents(
        descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
    ) -> &crate::dynamic_executable::import_sections::dynamic_sections::ElfDynamicSectionContents
    {
        dynamic_payloads(descriptors).plan().contents()
    }

    fn target(descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan) -> TargetProfile {
        dynamic_payloads(descriptors)
            .plan()
            .inputs()
            .interpreter()
            .target()
    }

    checked::seal_stage! {
        field: descriptors,
        contents: contents,
        validated: ValidatedElfDynamicTagPlan,
        validate: validate_contents,
        fingerprint: non_authoritative_tag_compatibility_fingerprint,
        message: "ELF dynamic-tag compatibility fingerprint does not replay",
    }

    fn validate_contents(
        descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
        contents: &ElfDynamicTagContents,
    ) -> Result<(), Diagnostic> {
        require(
            descriptors.descriptor_count() == 11,
            "ELF dynamic tags require the exact eleven-section descriptor carrier",
        )?;
        require(
            matches!(
                target(descriptors),
                TargetProfile::LinuxX64 | TargetProfile::LinuxArm64
            ),
            "ELF dynamic tags require an exact supported Linux profile",
        )?;
        let structural = structural_contents(descriptors);
        require(
            !structural.needed.is_empty(),
            "ELF dynamic tags require at least one owned DT_NEEDED row",
        )?;
        let expected_row_count = checked_sum(
            structural.needed.len(),
            UNVERSIONED_FIXED_ROW_COUNT + 3 * usize::from(!structural.verneed.is_empty()),
            "validated ELF dynamic row count",
        )?;
        require(
            contents.rows.len() == expected_row_count,
            "ELF dynamic tag row count is not exact",
        )?;
        validate_needed_rows(descriptors, contents, structural.needed.len())?;
        validate_fixed_rows(descriptors, contents, structural.needed.len())?;
        validate_descriptor_targets(descriptors)?;
        validate_address_obligations(contents, !structural.verneed.is_empty())?;
        validate_relocation_closure(descriptors)?;
        Ok(())
    }

    fn validate_descriptor_targets(
        descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
    ) -> Result<(), Diagnostic> {
        let appended = &descriptors.contents().descriptors;
        for kind in [
            ElfProcedureLinkageSectionKind::ProcedureGot,
            ElfProcedureLinkageSectionKind::ProcedureRelocation,
        ] {
            require(
                appended.iter().filter(|row| row.kind == kind).count() == 1,
                "ELF dynamic address target is not uniquely owned by the linkage descriptors",
            )?;
        }
        Ok(())
    }

    fn validate_needed_rows(
        descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
        contents: &ElfDynamicTagContents,
        needed_count: usize,
    ) -> Result<(), Diagnostic> {
        let structural = structural_contents(descriptors);
        let dynstr = &dynamic_payloads(descriptors).payloads().dynstr;
        let mut prior = None;
        for (index, expected_offset) in structural.needed.iter().enumerate() {
            let row = contents
                .rows
                .get(index)
                .ok_or_else(|| Diagnostic::error("missing DT_NEEDED row"))?;
            require(
                *row == (ElfDynamicSemanticRow {
                    tag: ElfDynamicTag::Needed,
                    value: ElfDynamicValue::NeededStringOffset(*expected_offset),
                }),
                "DT_NEEDED row drifted from the exact significant roster",
            )?;
            let object = name_at(dynstr, *expected_offset, "DT_NEEDED string offset")?;
            require(
                !object.is_empty(),
                "DT_NEEDED references an empty object name",
            )?;
            if let Some(prior_object) = prior {
                require(
                    prior_object < object,
                    "DT_NEEDED rows are duplicated or not in canonical significant order",
                )?;
            }
            prior = Some(object);
        }
        require(
            contents
                .rows
                .get(needed_count)
                .is_some_and(|row| row.tag != ElfDynamicTag::Needed),
            "DT_NEEDED rows are not one contiguous significant prefix",
        )
    }

    fn validate_fixed_rows(
        descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
        contents: &ElfDynamicTagContents,
        base: usize,
    ) -> Result<(), Diagnostic> {
        let structural = structural_contents(descriptors);
        let mut fixed = vec![
            ElfDynamicSemanticRow {
                tag: ElfDynamicTag::ProcedureRelocationSize,
                value: ElfDynamicValue::ProcedureRelocationByteCount(super::super::checked_u64(
                    descriptors.templates().procedure_relocation_byte_count(),
                    "validated procedure relocation size",
                )?),
            },
            address_row(ElfDynamicTag::ProcedureGot),
            address_row(ElfDynamicTag::SystemVHash),
            address_row(ElfDynamicTag::GnuHash),
            address_row(ElfDynamicTag::DynamicString),
            address_row(ElfDynamicTag::DynamicSymbol),
            ElfDynamicSemanticRow {
                tag: ElfDynamicTag::DynamicStringSize,
                value: ElfDynamicValue::DynamicStringByteCount(super::super::checked_u64(
                    dynamic_payloads(descriptors).dynamic_string_byte_count(),
                    "validated dynamic string size",
                )?),
            },
            ElfDynamicSemanticRow {
                tag: ElfDynamicTag::DynamicSymbolEntrySize,
                value: ElfDynamicValue::DynamicSymbolEntryByteCount(ELF64_DYNAMIC_SYMBOL_SIZE),
            },
            ElfDynamicSemanticRow {
                tag: ElfDynamicTag::ProcedureRelocationKind,
                value: ElfDynamicValue::RelocationTag(ElfDynamicTag::Rela),
            },
            address_row(ElfDynamicTag::ProcedureRelocation),
            address_row(ElfDynamicTag::Rela),
            ElfDynamicSemanticRow {
                tag: ElfDynamicTag::GeneralRelocationSize,
                value: ElfDynamicValue::GeneralRelocationByteCount(super::super::checked_u64(
                    descriptors.templates().general_relocation_byte_count(),
                    "validated general relocation size",
                )?),
            },
            ElfDynamicSemanticRow {
                tag: ElfDynamicTag::GeneralRelocationEntrySize,
                value: ElfDynamicValue::GeneralRelocationEntryByteCount(super::super::checked_u64(
                    ELF64_RELA_SIZE,
                    "validated general relocation entry size",
                )?),
            },
        ];
        if !structural.verneed.is_empty() {
            fixed.extend([
                address_row(ElfDynamicTag::GnuSymbolVersion),
                address_row(ElfDynamicTag::GnuVersionRequirement),
                ElfDynamicSemanticRow {
                    tag: ElfDynamicTag::GnuVersionRequirementCount,
                    value: ElfDynamicValue::VersionRequirementRecordCount(
                        super::super::checked_u64(
                            structural.verneed.len(),
                            "validated version-requirement count",
                        )?,
                    ),
                },
            ]);
        }
        fixed.push(ElfDynamicSemanticRow {
            tag: ElfDynamicTag::Flags,
            value: ElfDynamicValue::BindingFlags(DF_BIND_NOW),
        });
        fixed.push(ElfDynamicSemanticRow {
            tag: ElfDynamicTag::Null,
            value: ElfDynamicValue::Null,
        });
        require(
            contents.rows.get(base..) == Some(fixed.as_slice()),
            "fixed ELF dynamic tags are missing, duplicated, reordered, or malformed",
        )?;
        let relocation_bytes = descriptors.templates().procedure_relocation_byte_count();
        let relocation_count = descriptors
            .templates()
            .linkage()
            .procedure_relocation_count();
        require(
            relocation_bytes > 0
                && relocation_bytes.is_multiple_of(ELF64_RELA_SIZE)
                && relocation_bytes
                    == checked_product(
                        relocation_count,
                        ELF64_RELA_SIZE,
                        "validated PLT relocation byte count",
                    )?,
            "DT_PLTRELSZ does not match exact Elf64_Rela JUMP_SLOT rows",
        )?;
        require(
            structural.verneed.len()
                == dynamic_payloads(descriptors)
                    .plan()
                    .required_version_object_count(),
            "DT_VERNEEDNUM does not cover the exact versioned-object roster",
        )
    }

    const fn address_row(tag: ElfDynamicTag) -> ElfDynamicSemanticRow {
        ElfDynamicSemanticRow {
            tag,
            value: ElfDynamicValue::AddressPlaceholder,
        }
    }

    fn validate_address_obligations(
        contents: &ElfDynamicTagContents,
        has_version_requirements: bool,
    ) -> Result<(), Diagnostic> {
        let expected = [
            (
                ElfDynamicTag::ProcedureGot,
                ElfDynamicAddressTarget::ProcedureGot,
            ),
            (
                ElfDynamicTag::SystemVHash,
                ElfDynamicAddressTarget::SystemVHash,
            ),
            (ElfDynamicTag::GnuHash, ElfDynamicAddressTarget::GnuHash),
            (
                ElfDynamicTag::DynamicString,
                ElfDynamicAddressTarget::DynamicString,
            ),
            (
                ElfDynamicTag::DynamicSymbol,
                ElfDynamicAddressTarget::DynamicSymbol,
            ),
            (
                ElfDynamicTag::ProcedureRelocation,
                ElfDynamicAddressTarget::ProcedureRelocation,
            ),
            (
                ElfDynamicTag::Rela,
                ElfDynamicAddressTarget::GeneralRelocation,
            ),
            (
                ElfDynamicTag::GnuSymbolVersion,
                ElfDynamicAddressTarget::GnuSymbolVersion,
            ),
            (
                ElfDynamicTag::GnuVersionRequirement,
                ElfDynamicAddressTarget::GnuVersionRequirement,
            ),
        ];
        let expected_count = UNVERSIONED_ADDRESS_COUNT + 2 * usize::from(has_version_requirements);
        let expected = &expected[..expected_count];
        require(
            contents.address_obligations.len() == expected.len(),
            "ELF dynamic address-obligation count is not exact",
        )?;
        for (obligation, (tag, target)) in contents
            .address_obligations
            .iter()
            .zip(expected.iter().copied())
        {
            let row_index = usize::try_from(obligation.row_ordinal)
                .map_err(|_| Diagnostic::error("ELF dynamic row ordinal exceeds usize"))?;
            require(
                obligation.byte_width == ELF64_DYN_VALUE_SIZE
                    && obligation.target == target
                    && contents.rows.get(row_index) == Some(&address_row(tag)),
                "ELF dynamic address obligation drifted from its exact semantic row",
            )?;
        }
        for (index, row) in contents.rows.iter().enumerate() {
            let obligation_count = contents
                .address_obligations
                .iter()
                .filter(|obligation| obligation.row_ordinal as usize == index)
                .count();
            require(
                obligation_count
                    == usize::from(matches!(row.value, ElfDynamicValue::AddressPlaceholder)),
                "ELF dynamic row has a missing, duplicate, or orphan address obligation",
            )?;
        }
        Ok(())
    }

    fn validate_relocation_closure(
        descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
    ) -> Result<(), Diagnostic> {
        let linkage = descriptors.templates().linkage();
        require(
            checked_product(
                linkage.general_dynamic_relocation_count(),
                ELF64_RELA_SIZE,
                "general relocation table byte count",
            )? == descriptors.templates().general_relocation_byte_count(),
            "general relocation table drifts from its semantic relocation rows",
        )?;
        let future_dynamic = descriptors
            .templates()
            .contents()
            .fixups
            .iter()
            .filter(|fixup| {
                fixup.storage == ElfProcedureLinkageFixupStorage::GotPlt
                    && fixup.kind == ElfProcedureLinkageFixupKind::Absolute64
                    && fixup.target == ElfProcedureLinkageSemanticTarget::FutureDynamicSection
            })
            .count();
        require(
            future_dynamic
                == match target(descriptors) {
                    TargetProfile::LinuxX64 => 1,
                    TargetProfile::LinuxArm64 => 0,
                    _ => return Err(Diagnostic::error("unsupported relocation-closure target")),
                },
            "target GOT policy drifted from the future .dynamic section obligation",
        )
    }

    fn non_authoritative_tag_compatibility_fingerprint(
        descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
        contents: &ElfDynamicTagContents,
    ) -> u64 {
        let mut hash = checked::stage_fingerprint(
            b"omega.elf-dynamic-tags.v2",
            descriptors.non_authoritative_descriptor_compatibility_fingerprint(),
        );
        hash.bytes(&(contents.rows.len() as u64).to_le_bytes());
        for row in &contents.rows {
            hash.bytes(&(row.tag as i64).to_le_bytes());
            match row.value {
                ElfDynamicValue::NeededStringOffset(offset) => {
                    hash.byte(1);
                    hash.bytes(&offset.to_le_bytes());
                }
                ElfDynamicValue::ProcedureRelocationByteCount(count) => {
                    tagged_u64(&mut hash, 2, count)
                }
                ElfDynamicValue::GeneralRelocationByteCount(count) => {
                    tagged_u64(&mut hash, 9, count)
                }
                ElfDynamicValue::GeneralRelocationEntryByteCount(count) => {
                    tagged_u64(&mut hash, 10, count)
                }
                ElfDynamicValue::AddressPlaceholder => hash.byte(3),
                ElfDynamicValue::DynamicStringByteCount(count) => tagged_u64(&mut hash, 4, count),
                ElfDynamicValue::DynamicSymbolEntryByteCount(count) => {
                    tagged_u64(&mut hash, 5, count)
                }
                ElfDynamicValue::RelocationTag(tag) => {
                    hash.byte(6);
                    hash.bytes(&(tag as i64).to_le_bytes());
                }
                ElfDynamicValue::VersionRequirementRecordCount(count) => {
                    tagged_u64(&mut hash, 7, count)
                }
                ElfDynamicValue::BindingFlags(flags) => tagged_u64(&mut hash, 11, flags),
                ElfDynamicValue::Null => hash.byte(8),
            }
        }
        hash.bytes(&(contents.address_obligations.len() as u64).to_le_bytes());
        for obligation in &contents.address_obligations {
            hash.bytes(&obligation.row_ordinal.to_le_bytes());
            hash.byte(obligation.byte_width);
            hash.byte(obligation.target as u8);
        }
        hash.finish()
    }

    fn tagged_u64(hash: &mut Fnv1a, tag: u8, value: u64) {
        hash.byte(tag);
        hash.bytes(&value.to_le_bytes());
    }

    #[cfg(test)]
    mod tests {
        use super::{
            Candidate, ElfDynamicAddressObligation, ElfDynamicAddressTarget, ElfDynamicSemanticRow,
            ElfDynamicTag, ElfDynamicValue, TargetProfile,
            ValidatedElfProcedureLinkageSectionDescriptorPlan, checked_product, checked_sum,
            checked_u32, derive_contents, non_authoritative_tag_compatibility_fingerprint,
            plan_elf_dynamic_tags, structural_contents, validate_candidate, validate_contents,
        };

        use crate::bytes::name_at;

        const IMPORTS: [crate::test_support::DynamicImportFixture; 2] =
            crate::test_support::PAIR_IMPORTS;

        use crate::test_support::DynamicImportFixture as ImportFixture;
        fn descriptors(
            target: TargetProfile,
            imports: &[ImportFixture],
        ) -> ValidatedElfProcedureLinkageSectionDescriptorPlan {
            let inputs = crate::test_support::dynamic_import_inputs(
                target,
                imports,
                "__omega_dynamic_tag_import_",
                "valid dynamic-tag locator",
                "valid dynamic-tag interpreter",
            );
            crate::test_support::linkage_descriptors(inputs)
        }

        fn address_obligation(
            row_ordinal: u32,
            target: ElfDynamicAddressTarget,
        ) -> ElfDynamicAddressObligation {
            ElfDynamicAddressObligation {
                row_ordinal,
                byte_width: 8,
                target,
            }
        }

        fn candidate(target: TargetProfile) -> Candidate {
            let descriptors = descriptors(target, &IMPORTS);
            let contents = derive_contents(&descriptors).expect("derived dynamic tags");
            let non_authoritative_tag_compatibility_fingerprint =
                non_authoritative_tag_compatibility_fingerprint(&descriptors, &contents);
            Candidate {
                descriptors,
                contents,
                non_authoritative_tag_compatibility_fingerprint,
            }
        }

        #[test]
        fn both_targets_plan_exact_needed_prefix_fixed_tags_and_address_obligations() {
            for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
                let plan = plan_elf_dynamic_tags(descriptors(target, &IMPORTS))
                    .expect("validated semantic dynamic tags");
                assert_eq!(plan.descriptors().descriptor_count(), 11);
                assert_eq!(plan.needed_row_count(), 2);
                assert_eq!(plan.row_count(), 20);
                assert_eq!(plan.address_obligation_count(), 9);
                assert_ne!(plan.non_authoritative_tag_compatibility_fingerprint(), 0);

                let structural = structural_contents(&plan.descriptors);
                assert_eq!(
                    &plan.contents.rows[..2],
                    &[
                        ElfDynamicSemanticRow {
                            tag: ElfDynamicTag::Needed,
                            value: ElfDynamicValue::NeededStringOffset(structural.needed[0]),
                        },
                        ElfDynamicSemanticRow {
                            tag: ElfDynamicTag::Needed,
                            value: ElfDynamicValue::NeededStringOffset(structural.needed[1]),
                        },
                    ],
                );
                assert_eq!(
                    plan.contents
                        .rows
                        .iter()
                        .map(|row| row.tag)
                        .collect::<Vec<_>>(),
                    [
                        ElfDynamicTag::Needed,
                        ElfDynamicTag::Needed,
                        ElfDynamicTag::ProcedureRelocationSize,
                        ElfDynamicTag::ProcedureGot,
                        ElfDynamicTag::SystemVHash,
                        ElfDynamicTag::GnuHash,
                        ElfDynamicTag::DynamicString,
                        ElfDynamicTag::DynamicSymbol,
                        ElfDynamicTag::DynamicStringSize,
                        ElfDynamicTag::DynamicSymbolEntrySize,
                        ElfDynamicTag::ProcedureRelocationKind,
                        ElfDynamicTag::ProcedureRelocation,
                        ElfDynamicTag::Rela,
                        ElfDynamicTag::GeneralRelocationSize,
                        ElfDynamicTag::GeneralRelocationEntrySize,
                        ElfDynamicTag::GnuSymbolVersion,
                        ElfDynamicTag::GnuVersionRequirement,
                        ElfDynamicTag::GnuVersionRequirementCount,
                        ElfDynamicTag::Flags,
                        ElfDynamicTag::Null,
                    ],
                );
                assert_eq!(
                    plan.contents.address_obligations,
                    [
                        address_obligation(3, ElfDynamicAddressTarget::ProcedureGot),
                        address_obligation(4, ElfDynamicAddressTarget::SystemVHash),
                        address_obligation(5, ElfDynamicAddressTarget::GnuHash),
                        address_obligation(6, ElfDynamicAddressTarget::DynamicString),
                        address_obligation(7, ElfDynamicAddressTarget::DynamicSymbol),
                        address_obligation(11, ElfDynamicAddressTarget::ProcedureRelocation),
                        address_obligation(12, ElfDynamicAddressTarget::GeneralRelocation),
                        address_obligation(15, ElfDynamicAddressTarget::GnuSymbolVersion),
                        address_obligation(16, ElfDynamicAddressTarget::GnuVersionRequirement),
                    ],
                );
                assert_eq!(
                    plan.contents.rows[2].value,
                    ElfDynamicValue::ProcedureRelocationByteCount(48),
                );
                assert_eq!(
                    plan.contents.rows[9].value,
                    ElfDynamicValue::DynamicSymbolEntryByteCount(24),
                );
                assert_eq!(
                    plan.contents.rows[10].value,
                    ElfDynamicValue::RelocationTag(ElfDynamicTag::Rela),
                );
                assert_eq!(
                    plan.contents.rows[12].value,
                    ElfDynamicValue::AddressPlaceholder,
                );
                assert_eq!(
                    plan.contents.rows[13].value,
                    ElfDynamicValue::GeneralRelocationByteCount(0),
                );
                assert_eq!(
                    plan.contents.rows[14].value,
                    ElfDynamicValue::GeneralRelocationEntryByteCount(24),
                );
                assert_eq!(
                    plan.contents.rows[17].value,
                    ElfDynamicValue::VersionRequirementRecordCount(2),
                );
                assert_eq!(
                    plan.contents.rows[18].value,
                    ElfDynamicValue::BindingFlags(0x8)
                );
                assert_eq!(plan.contents.rows[19].value, ElfDynamicValue::Null);
                validate_contents(plan.descriptors(), &plan.contents)
                    .expect("independent dynamic-tag replay");
            }
        }

        #[test]
        fn import_permutation_preserves_rows_and_identity_while_target_stays_bound() {
            let forward =
                plan_elf_dynamic_tags(descriptors(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
            let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
            let reverse =
                plan_elf_dynamic_tags(descriptors(TargetProfile::LinuxX64, &reverse_imports))
                    .unwrap();
            let arm =
                plan_elf_dynamic_tags(descriptors(TargetProfile::LinuxArm64, &IMPORTS)).unwrap();
            assert_eq!(forward.contents, reverse.contents);
            assert_eq!(
                forward.non_authoritative_tag_compatibility_fingerprint(),
                reverse.non_authoritative_tag_compatibility_fingerprint()
            );
            assert_ne!(
                forward.non_authoritative_tag_compatibility_fingerprint(),
                arm.non_authoritative_tag_compatibility_fingerprint()
            );
        }

        #[test]
        fn exact_tag_set_binds_general_relocation_and_omits_optional_policies() {
            let plan =
                plan_elf_dynamic_tags(descriptors(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
            assert_eq!(
                plan.contents
                    .rows
                    .iter()
                    .filter(|row| row.tag == ElfDynamicTag::Rela)
                    .count(),
                1,
            );
            assert_eq!(
                plan.descriptors
                    .templates()
                    .linkage()
                    .general_dynamic_relocation_count(),
                0,
            );
            assert_eq!(
                plan.contents
                    .rows
                    .iter()
                    .filter(|row| row.tag == ElfDynamicTag::GeneralRelocationEntrySize)
                    .count(),
                1,
            );
            assert_eq!(
                plan.contents
                    .rows
                    .iter()
                    .filter(|row| row.tag == ElfDynamicTag::Null)
                    .count(),
                1,
            );
            assert_eq!(plan.contents.rows.last().unwrap().tag, ElfDynamicTag::Null);
        }

        #[test]
        fn independent_replay_rejects_every_value_family_order_and_identity_corruption() {
            let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
                Box::new(|candidate| candidate.contents.rows.swap(0, 1)),
                Box::new(|candidate| candidate.contents.rows.swap(2, 3)),
                Box::new(|candidate| {
                    candidate.contents.rows.pop();
                }),
                Box::new(|candidate| candidate.contents.rows.push(candidate.contents.rows[18])),
                Box::new(|candidate| candidate.contents.rows[0].tag = ElfDynamicTag::Null),
                Box::new(|candidate| {
                    candidate.contents.rows[0].value = ElfDynamicValue::NeededStringOffset(0)
                }),
                Box::new(|candidate| {
                    candidate.contents.rows[2].value =
                        ElfDynamicValue::ProcedureRelocationByteCount(47)
                }),
                Box::new(|candidate| candidate.contents.rows[3].value = ElfDynamicValue::Null),
                Box::new(|candidate| {
                    candidate.contents.rows[8].value = ElfDynamicValue::DynamicStringByteCount(0)
                }),
                Box::new(|candidate| {
                    candidate.contents.rows[9].value =
                        ElfDynamicValue::DynamicSymbolEntryByteCount(16)
                }),
                Box::new(|candidate| {
                    candidate.contents.rows[10].value =
                        ElfDynamicValue::RelocationTag(ElfDynamicTag::Null)
                }),
                Box::new(|candidate| {
                    candidate.contents.rows[14].value =
                        ElfDynamicValue::GeneralRelocationEntryByteCount(16)
                }),
                Box::new(|candidate| {
                    candidate.contents.rows[17].value =
                        ElfDynamicValue::VersionRequirementRecordCount(1)
                }),
                Box::new(|candidate| {
                    candidate.contents.rows[18].value = ElfDynamicValue::AddressPlaceholder
                }),
                Box::new(|candidate| {
                    candidate.non_authoritative_tag_compatibility_fingerprint ^= 1
                }),
            ];
            for corrupt in corruptions {
                let mut candidate = candidate(TargetProfile::LinuxX64);
                let expected_identity = candidate
                    .descriptors
                    .non_authoritative_descriptor_compatibility_fingerprint();
                corrupt(&mut candidate);
                let error = validate_candidate(candidate)
                    .expect_err("corrupt semantic dynamic tags must reject");
                assert_eq!(
                    error
                        .candidate
                        .descriptors
                        .non_authoritative_descriptor_compatibility_fingerprint(),
                    expected_identity,
                    "dynamic-tag rejection retains exact descriptor custody",
                );
            }
        }

        #[test]
        fn independent_replay_rejects_missing_duplicate_or_misdirected_address_obligations() {
            let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
                Box::new(|candidate| {
                    candidate.contents.address_obligations.pop();
                }),
                Box::new(|candidate| {
                    candidate
                        .contents
                        .address_obligations
                        .push(candidate.contents.address_obligations[0])
                }),
                Box::new(|candidate| candidate.contents.address_obligations.swap(0, 1)),
                Box::new(|candidate| {
                    candidate.contents.address_obligations[0].row_ordinal = u32::MAX
                }),
                Box::new(|candidate| candidate.contents.address_obligations[0].byte_width = 4),
                Box::new(|candidate| {
                    candidate.contents.address_obligations[0].target =
                        ElfDynamicAddressTarget::DynamicString
                }),
            ];
            for corrupt in corruptions {
                let mut candidate = candidate(TargetProfile::LinuxArm64);
                let expected_identity = candidate
                    .descriptors
                    .non_authoritative_descriptor_compatibility_fingerprint();
                corrupt(&mut candidate);
                let error = validate_candidate(candidate)
                    .expect_err("corrupt dynamic address obligations must reject");
                assert_eq!(
                    error
                        .candidate
                        .descriptors
                        .non_authoritative_descriptor_compatibility_fingerprint(),
                    expected_identity,
                );
            }
        }

        #[test]
        fn malformed_offsets_counts_and_arithmetic_reject_without_panicking() {
            assert!(checked_sum(usize::MAX, 1, "sum").is_err());
            assert!(checked_product(usize::MAX, 2, "product").is_err());
            assert!(checked_u32(usize::MAX, "word").is_err());
            assert!(name_at(&[], 0, "DT_NEEDED string offset").is_err());
            assert!(name_at(b"unterminated", 0, "DT_NEEDED string offset").is_err());
            assert!(name_at(b"\0", u32::MAX, "DT_NEEDED string offset").is_err());

            let mut candidate = candidate(TargetProfile::LinuxX64);
            candidate.contents.address_obligations[0].row_ordinal = u32::MAX;
            assert!(validate_candidate(candidate).is_err());
        }

        #[test]
        fn unversioned_component_remains_needed_without_inventing_a_gnu_version() {
            let profile = TargetProfile::LinuxX64;
            let inputs = crate::test_support::dynamic_import_inputs(
                profile,
                &IMPORTS,
                "__omega_mixed_import_",
                "valid locator",
                "valid interpreter",
            );
            let (mut image, interpreter, _) = inputs.into_parts();
            let locator =
                target::NormalizedComponentLocator::for_image(profile, b"component bytes", [7; 32])
                    .expect("exact component locator");
            image.memory.text[48] = 0xe8;
            crate::test_support::import_with_text_relocations(
                &mut image,
                "__omega_component",
                image::FinalImageImportPlan::Normalized(locator.into()),
                &[49],
            );
            let inputs = crate::plan_elf_dynamic_link_inputs(image, interpreter).unwrap();
            let descriptors = crate::test_support::linkage_descriptors(inputs);
            let plan =
                plan_elf_dynamic_tags(descriptors).expect("mixed versioned and component imports");
            assert_eq!(plan.needed_row_count(), 3);
            assert_eq!(structural_contents(plan.descriptors()).verneed.len(), 2);
            let sections = plan.descriptors().templates().linkage().descriptors();
            let requirements = sections.contents().descriptors.iter().find(|row|
                row.kind == crate::dynamic_executable::import_sections::dynamic_section_descriptors::ElfDynamicSectionKind::GnuVersionRequirement).unwrap();
            assert_eq!(
                requirements.info, 2,
                "section sh_info counts Verneed records, not DT_NEEDED objects"
            );

            let count_row = plan
                .contents
                .rows
                .iter()
                .position(|row| row.tag == ElfDynamicTag::GnuVersionRequirementCount)
                .unwrap();
            assert_eq!(
                plan.contents.rows[count_row].value,
                ElfDynamicValue::VersionRequirementRecordCount(2)
            );
            for incorrect_count in [1, 3] {
                let mut malformed = plan.contents.clone();
                malformed.rows[count_row].value =
                    ElfDynamicValue::VersionRequirementRecordCount(incorrect_count);
                assert!(
                    validate_contents(plan.descriptors(), &malformed).is_err(),
                    "missing/extra version-need counts must still reject"
                );
            }
        }

        #[test]
        fn component_only_imports_omit_absent_version_records_and_their_address_fixup() {
            let profile = TargetProfile::LinuxX64;
            let mut image = image::FinalImage::with_capacity(
                profile.native_target(),
                image::FinalImageMemory {
                    text: vec![0; 64],
                    ..image::FinalImageMemory::default()
                },
                arena::Handle::invalid(),
                2,
                2,
                2,
            );
            for index in 0..2 {
                let locator = target::NormalizedComponentLocator::for_image(
                    profile,
                    &[index as u8],
                    [index as u8 + 1; 32],
                )
                .unwrap();
                let offset = index * 16;
                image.memory.text[offset] = 0xe8;
                crate::test_support::import_with_text_relocations(
                    &mut image,
                    &format!("__omega_component_{index}"),
                    image::FinalImageImportPlan::Normalized(locator.into()),
                    &[offset + 1],
                );
            }
            let inputs = crate::plan_elf_dynamic_link_inputs(
                image,
                crate::test_support::interpreter(profile),
            )
            .unwrap();
            let descriptors = crate::test_support::linkage_descriptors(inputs);
            let plan = plan_elf_dynamic_tags(descriptors).unwrap();
            assert_eq!(plan.needed_row_count(), 2);
            assert_eq!(plan.row_count(), 17);
            assert_eq!(plan.address_obligation_count(), 7);
            assert!(!plan.has_version_requirements());
            assert!(!plan.contents.rows.iter().any(|row| matches!(
                row.tag,
                ElfDynamicTag::GnuSymbolVersion
                    | ElfDynamicTag::GnuVersionRequirement
                    | ElfDynamicTag::GnuVersionRequirementCount
            )));
            let mut symbol_only = plan.contents.clone();
            let terminal = symbol_only.rows.pop().unwrap();
            symbol_only.rows.extend([
                super::address_row(ElfDynamicTag::GnuSymbolVersion),
                terminal,
            ]);
            assert!(
                validate_contents(plan.descriptors(), &symbol_only).is_err(),
                "an unversioned executable cannot advertise a loader version table"
            );
            let mut malformed = plan.contents.clone();
            let terminal = malformed.rows.pop().unwrap();
            malformed.rows.extend([
                super::address_row(ElfDynamicTag::GnuVersionRequirement),
                ElfDynamicSemanticRow {
                    tag: ElfDynamicTag::GnuVersionRequirementCount,
                    value: ElfDynamicValue::VersionRequirementRecordCount(0),
                },
                terminal,
            ]);
            assert!(
                validate_contents(plan.descriptors(), &malformed).is_err(),
                "a zero count cannot justify a present pointer to an absent Verneed record"
            );
        }

        #[test]
        fn finite_program_stack_requires_loader_binding_before_entry() {
            let plan =
                plan_elf_dynamic_tags(descriptors(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
            let mut unbound = plan.contents.clone();
            let flags = unbound
                .rows
                .iter_mut()
                .find(|row| row.tag == ElfDynamicTag::Flags)
                .unwrap();
            flags.value = ElfDynamicValue::BindingFlags(0);
            assert!(validate_contents(plan.descriptors(), &unbound).is_err());
            unbound.rows.retain(|row| row.tag != ElfDynamicTag::Flags);
            assert!(validate_contents(plan.descriptors(), &unbound).is_err());
        }
    }
}

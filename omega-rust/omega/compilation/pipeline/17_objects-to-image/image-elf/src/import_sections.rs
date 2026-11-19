/*
We execute stages 1 through 4 of the dynamic ELF pipeline: preflighting
dynamic-link inputs (`plan_elf_dynamic_link_inputs`), planning `.interp`,
`.dynstr`, `.dynsym`, `.gnu.version`, `.gnu.version_r`, and `.rela.dyn`
(`plan_elf_dynamic_sections`), serializing their payloads
(`serialize_elf_dynamic_sections`), and planning their section descriptors
(`plan_elf_dynamic_section_descriptors`).
*/

pub(crate) mod dynamic_link {
    use crate::imports::{ElfImportLocator, ElfImportRequest, canonical_referenced_imports};
    use diagnostics::Diagnostic;
    use image::FinalImage;
    use target::{NormalizedElfInterpreterPlan, TargetProfile};

    #[derive(Debug)]
    #[must_use = "planned ELF dynamic-link inputs retain the exact final image and interpreter"]
    pub struct PlannedElfDynamicLinkInputs {
        image: FinalImage,
        interpreter: NormalizedElfInterpreterPlan,
        imports: Vec<ElfImportRequest>,
    }

    impl PlannedElfDynamicLinkInputs {
        pub const fn image(&self) -> &FinalImage {
            &self.image
        }

        pub const fn interpreter(&self) -> &NormalizedElfInterpreterPlan {
            &self.interpreter
        }

        pub fn referenced_import_count(&self) -> usize {
            self.imports.len()
        }

        pub(crate) fn imports(&self) -> &[ElfImportRequest] {
            &self.imports
        }

        pub(crate) fn into_parts(
            self,
        ) -> (
            FinalImage,
            NormalizedElfInterpreterPlan,
            Vec<ElfImportRequest>,
        ) {
            (self.image, self.interpreter, self.imports)
        }
    }

    #[derive(Debug)]
    #[must_use = "ELF dynamic-link rejection retains the final image and interpreter input"]
    pub struct ElfDynamicLinkInputPlanningError {
        image: FinalImage,
        interpreter: NormalizedElfInterpreterPlan,
        diagnostic: Diagnostic,
    }

    image::rejection_carrier!(
        ElfDynamicLinkInputPlanningError;
        image: FinalImage,
        interpreter: NormalizedElfInterpreterPlan,
    );

    pub(crate) fn plan_elf_dynamic_link_inputs(
        image: FinalImage,
        interpreter: NormalizedElfInterpreterPlan,
    ) -> Result<PlannedElfDynamicLinkInputs, Box<ElfDynamicLinkInputPlanningError>> {
        let reject = |image, interpreter, diagnostic| {
            Err(Box::new(ElfDynamicLinkInputPlanningError {
                image,
                interpreter,
                diagnostic,
            }))
        };

        if interpreter.target().native_target() != image.target {
            return reject(
                image,
                interpreter,
                Diagnostic::error(
                    "ELF interpreter profile does not match the exact final-image target",
                ),
            );
        }

        let imports = match canonical_referenced_imports(&image) {
            Ok(imports) => imports,
            Err(diagnostic) => return reject(image, interpreter, diagnostic),
        };
        if imports.is_empty() {
            return reject(
                image,
                interpreter,
                Diagnostic::error(
                    "ELF interpreter input has no referenced normalized versioned import to join",
                ),
            );
        }

        for import in &imports {
            let target_profile = match &import.locator {
                ElfImportLocator::Versioned { target_profile, .. } => *target_profile,
                ElfImportLocator::Component { locator, .. } => locator.target(),
                ElfImportLocator::StringBackedBootstrap { .. } => {
                    return reject(
                        image,
                        interpreter,
                        Diagnostic::error(
                            "string-backed ELF bootstrap imports cannot enter normalized dynamic-link planning",
                        ),
                    );
                }
            };
            if target_profile != interpreter.target()
                || !matches!(
                    target_profile,
                    TargetProfile::LinuxArm64 | TargetProfile::LinuxX64
                )
            {
                return reject(
                    image,
                    interpreter,
                    Diagnostic::error(
                        "versioned ELF import profile does not match the exact interpreter profile",
                    ),
                );
            }
        }

        Ok(PlannedElfDynamicLinkInputs {
            image,
            interpreter,
            imports,
        })
    }

    #[cfg(test)]
    mod tests {
        use super::{
            ElfImportLocator, FinalImage, NormalizedElfInterpreterPlan, TargetProfile,
            plan_elf_dynamic_link_inputs,
        };
        use arena::Handle;
        use image::{FinalImageImportPlan, FinalImageMemory};
        use target::{
            ForeignLocatorCandidate, NativeTarget, normalize_elf_interpreter_plan,
            normalize_foreign_locator,
        };

        fn interpreter(target: TargetProfile, path: &[u8]) -> NormalizedElfInterpreterPlan {
            normalize_elf_interpreter_plan(path.to_vec(), target).expect("valid interpreter input")
        }

        fn image_with_import(import: FinalImageImportPlan) -> FinalImage {
            let target = match &import {
                FinalImageImportPlan::Normalized(locator) => locator.target().native_target(),
                FinalImageImportPlan::StringBackedBootstrap { .. } | FinalImageImportPlan::None => {
                    NativeTarget::linux_x64()
                }
            };
            let mut image = crate::test_support::relocation_image(target, 20);
            crate::test_support::import_with_text_relocations(
                &mut image,
                "__omega_dynamic_import",
                import,
                &[3, 15],
            );
            image
        }

        fn versioned_import(
            target: TargetProfile,
            object: &[u8],
            symbol: &[u8],
            version: &[u8],
        ) -> FinalImageImportPlan {
            FinalImageImportPlan::Normalized(
                normalize_foreign_locator(
                    ForeignLocatorCandidate::ElfVersioned {
                        object: object.to_vec(),
                        symbol: symbol.to_vec(),
                        version: version.to_vec(),
                    },
                    target,
                )
                .expect("valid versioned ELF locator")
                .into(),
            )
        }

        #[test]
        fn exact_interpreter_and_canonical_versioned_import_rows_join_without_mutation() {
            let object = b"libraw\xff.so.6";
            let symbol = b"entry\xfe";
            let version = b"ABI_4\xfd";
            let image = image_with_import(versioned_import(
                TargetProfile::LinuxX64,
                object,
                symbol,
                version,
            ));
            let expected_image = image.clone();
            let interpreter =
                interpreter(TargetProfile::LinuxX64, b"/lib64/ld-linux-\xfc-x86-64.so.2");
            let expected_interpreter = interpreter.clone();

            let planned = plan_elf_dynamic_link_inputs(image, interpreter)
                .expect("matching exact dynamic-link inputs");

            assert_eq!(planned.image(), &expected_image);
            assert_eq!(planned.interpreter(), &expected_interpreter);
            assert_eq!(planned.referenced_import_count(), 1);
            assert_eq!(
                planned.imports[0].locator,
                ElfImportLocator::Versioned {
                    target_profile: TargetProfile::LinuxX64,
                    compatibility_report_identity: match &expected_image
                        .symbol_table
                        .imports
                        .iter()
                        .next()
                        .expect("one import")
                        .1
                        .import
                    {
                        FinalImageImportPlan::Normalized(locator) => {
                            locator.non_authoritative_compatibility_fingerprint()
                        }
                        _ => unreachable!("fixture uses a normalized import"),
                    },
                    object: object.to_vec(),
                    symbol: symbol.to_vec(),
                    version: version.to_vec(),
                },
            );
            assert_eq!(
                planned.imports[0]
                    .relocations
                    .iter()
                    .map(|relocation| relocation.offset)
                    .collect::<Vec<_>>(),
                [3, 15],
            );
        }

        #[test]
        fn target_drift_rejects_and_returns_both_original_inputs() {
            let image = image_with_import(versioned_import(
                TargetProfile::LinuxX64,
                b"libexact.so.1",
                b"entry",
                b"ABI_1",
            ));
            let expected_image = image.clone();
            let interpreter = interpreter(TargetProfile::LinuxArm64, b"/lib/ld-linux-aarch64.so.1");
            let expected_interpreter = interpreter.clone();

            let error = plan_elf_dynamic_link_inputs(image, interpreter)
                .expect_err("different architecture/profile must reject");
            let (returned_image, returned_interpreter, diagnostic) = error.into_parts();

            assert_eq!(returned_image, expected_image);
            assert_eq!(returned_interpreter, expected_interpreter);
            assert!(diagnostic.message.contains("final-image target"));
        }

        #[test]
        fn unused_interpreter_and_string_bootstrap_import_reject_recoverably() {
            let empty = FinalImage::with_capacity(
                NativeTarget::linux_x64(),
                FinalImageMemory::default(),
                Handle::invalid(),
                0,
                0,
                0,
            );
            let empty_expected = empty.clone();
            let empty_interpreter = interpreter(TargetProfile::LinuxX64, b"/loader");
            let empty_interpreter_expected = empty_interpreter.clone();
            let error = plan_elf_dynamic_link_inputs(empty, empty_interpreter)
                .expect_err("an unused interpreter input must reject");
            let (returned_image, returned_interpreter, diagnostic) = error.into_parts();
            assert_eq!(returned_image, empty_expected);
            assert_eq!(returned_interpreter, empty_interpreter_expected);
            assert!(diagnostic.message.contains("no referenced"));

            let bootstrap = image_with_import(FinalImageImportPlan::StringBackedBootstrap {
                library: "libbootstrap.so".into(),
            });
            let bootstrap_expected = bootstrap.clone();
            let bootstrap_interpreter = interpreter(TargetProfile::LinuxX64, b"/loader");
            let bootstrap_interpreter_expected = bootstrap_interpreter.clone();
            let error = plan_elf_dynamic_link_inputs(bootstrap, bootstrap_interpreter)
                .expect_err("string-backed bootstrap rows must not enter the normalized plan");
            let (returned_image, returned_interpreter, diagnostic) = error.into_parts();
            assert_eq!(returned_image, bootstrap_expected);
            assert_eq!(returned_interpreter, bootstrap_interpreter_expected);
            assert!(diagnostic.message.contains("string-backed"));
        }

        #[test]
        fn canonical_import_rejection_returns_unmodified_inputs() {
            let locator = normalize_foreign_locator(
                ForeignLocatorCandidate::ElfVersioned {
                    object: b"libexact.so.1".to_vec(),
                    symbol: b"entry".to_vec(),
                    version: b"ABI_1".to_vec(),
                },
                TargetProfile::LinuxX64,
            )
            .expect("valid locator");
            let mut image =
                image_with_import(FinalImageImportPlan::Normalized((locator.clone()).into()));
            let duplicated = image
                .symbol_table
                .imports
                .iter()
                .next()
                .expect("one import")
                .1
                .clone();
            image.symbol_table.imports.insert(duplicated);
            let expected_image = image.clone();
            let interpreter = interpreter(TargetProfile::LinuxX64, b"/loader");
            let expected_interpreter = interpreter.clone();

            let error = plan_elf_dynamic_link_inputs(image, interpreter)
                .expect_err("duplicate canonical locator rows must reject");
            let (returned_image, returned_interpreter, diagnostic) = error.into_parts();

            assert_eq!(returned_image, expected_image);
            assert_eq!(returned_interpreter, expected_interpreter);
            assert!(diagnostic.message.contains("duplicate import rows"));
        }
    }
}

pub(crate) mod dynamic_section_bytes {
    use target::compatibility_fingerprint::Fnv1a;

    use super::super::checked::{self, CandidateValidationError};
    use crate::bytes::{name_at, write_u16, write_u32, write_u64};
    use crate::dynamic_executable::checked::checked_product;
    use crate::dynamic_executable::checked::{
        checked_sum, read_u8, read_u16, read_u32, read_u64, require,
    };
    use crate::dynamic_executable::import_sections::dynamic_sections::{
        ElfDynamicSectionContents, ElfDynamicSymbol, ElfGnuHash, ElfSysvHash, ElfVersionNeed,
        ElfVersionNeedAuxiliary, ValidatedElfDynamicSectionPlan,
    };
    use diagnostics::Diagnostic;

    const ELF64_SYMBOL_SIZE: usize = 24;
    const ELF64_VERSION_NEED_SIZE: usize = 16;
    const ELF64_VERSION_AUXILIARY_SIZE: usize = 16;
    #[derive(Debug)]
    #[must_use = "validated ELF payloads retain the exact structural section plan"]
    pub struct ValidatedElfDynamicSectionPayloads {
        plan: ValidatedElfDynamicSectionPlan,
        payloads: ElfDynamicSectionPayloadBytes,
        non_authoritative_payload_compatibility_fingerprint: u64,
    }

    impl ValidatedElfDynamicSectionPayloads {
        pub const fn plan(&self) -> &ValidatedElfDynamicSectionPlan {
            &self.plan
        }

        pub fn interpreter_byte_count(&self) -> usize {
            self.payloads.interpreter.len()
        }

        pub fn dynamic_string_byte_count(&self) -> usize {
            self.payloads.dynstr.len()
        }

        #[cfg(test)]
        pub(crate) fn dynamic_symbol_byte_count(&self) -> usize {
            self.payloads.dynsym.len()
        }

        #[cfg(test)]
        pub(crate) fn system_v_hash_byte_count(&self) -> usize {
            self.payloads.sysv_hash.len()
        }

        #[cfg(test)]
        pub(crate) fn gnu_hash_byte_count(&self) -> usize {
            self.payloads.gnu_hash.len()
        }

        #[cfg(test)]
        pub(crate) fn symbol_version_byte_count(&self) -> usize {
            self.payloads.versym.len()
        }

        #[cfg(test)]
        pub(crate) fn version_requirement_byte_count(&self) -> usize {
            self.payloads.verneed.len()
        }

        pub const fn non_authoritative_payload_compatibility_fingerprint(&self) -> u64 {
            self.non_authoritative_payload_compatibility_fingerprint
        }

        pub(crate) const fn payloads(&self) -> &ElfDynamicSectionPayloadBytes {
            &self.payloads
        }

        pub(crate) fn into_parts(
            self,
        ) -> (
            ValidatedElfDynamicSectionPlan,
            ElfDynamicSectionPayloadBytes,
        ) {
            (self.plan, self.payloads)
        }
    }

    #[derive(Debug)]
    #[must_use = "ELF payload serialization rejection retains the validated structural plan"]
    pub struct ElfDynamicSectionSerializationError {
        plan: ValidatedElfDynamicSectionPlan,
        diagnostic: Diagnostic,
    }

    image::rejection_carrier!(
        ElfDynamicSectionSerializationError;
        plan: ValidatedElfDynamicSectionPlan,
    );

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ElfDynamicSectionPayloadBytes {
        pub(crate) interpreter: Vec<u8>,
        pub(crate) dynstr: Vec<u8>,
        pub(crate) dynsym: Vec<u8>,
        pub(crate) sysv_hash: Vec<u8>,
        pub(crate) gnu_hash: Vec<u8>,
        pub(crate) versym: Vec<u8>,
        pub(crate) verneed: Vec<u8>,
    }

    struct Candidate {
        plan: ValidatedElfDynamicSectionPlan,
        payloads: ElfDynamicSectionPayloadBytes,
        non_authoritative_payload_compatibility_fingerprint: u64,
    }

    pub(crate) fn serialize_elf_dynamic_sections(
        plan: ValidatedElfDynamicSectionPlan,
    ) -> Result<ValidatedElfDynamicSectionPayloads, Box<ElfDynamicSectionSerializationError>> {
        checked::plan_stage!(
            plan: plan,
            derive: |plan| encode_payloads(plan.contents()),
            contents: payloads,
            fingerprint: non_authoritative_payload_compatibility_fingerprint,
            error: ElfDynamicSectionSerializationError,
        )
    }

    fn encode_payloads(
        contents: &ElfDynamicSectionContents,
    ) -> Result<ElfDynamicSectionPayloadBytes, Diagnostic> {
        let mut dynsym = Vec::with_capacity(checked_product(
            contents.dynsym.len(),
            ELF64_SYMBOL_SIZE,
            "ELF64 dynamic symbol payload size",
        )?);
        for symbol in &contents.dynsym {
            write_u32(&mut dynsym, symbol.name);
            dynsym.push(symbol.info);
            dynsym.push(symbol.other);
            write_u16(&mut dynsym, symbol.section_index);
            write_u64(&mut dynsym, symbol.value);
            write_u64(&mut dynsym, symbol.size);
        }

        let hash_word_count = 2usize
            .checked_add(contents.sysv_hash.buckets.len())
            .and_then(|count| count.checked_add(contents.sysv_hash.chains.len()))
            .ok_or_else(|| Diagnostic::error("System V hash payload word count overflow"))?;
        let mut sysv_hash = Vec::with_capacity(checked_product(
            hash_word_count,
            4,
            "System V hash payload size",
        )?);
        write_u32(&mut sysv_hash, contents.sysv_hash.bucket_count);
        write_u32(&mut sysv_hash, contents.sysv_hash.chain_count);
        for word in contents
            .sysv_hash
            .buckets
            .iter()
            .chain(&contents.sysv_hash.chains)
        {
            write_u32(&mut sysv_hash, *word);
        }

        let gnu_hash_bloom_size = checked_product(
            contents.gnu_hash.bloom.len(),
            8,
            "GNU hash bloom payload size",
        )?;
        let gnu_hash_bucket_size = checked_product(
            contents.gnu_hash.buckets.len(),
            4,
            "GNU hash bucket payload size",
        )?;
        let gnu_hash_chain_size = checked_product(
            contents.gnu_hash.chains.len(),
            4,
            "GNU hash chain payload size",
        )?;
        let gnu_hash_size = checked_sum(16, gnu_hash_bloom_size, "GNU hash bloom payload end")?;
        let gnu_hash_size = checked_sum(
            gnu_hash_size,
            gnu_hash_bucket_size,
            "GNU hash bucket payload end",
        )?;
        let gnu_hash_size = checked_sum(
            gnu_hash_size,
            gnu_hash_chain_size,
            "GNU hash chain payload end",
        )?;
        let mut gnu_hash = Vec::with_capacity(gnu_hash_size);
        write_u32(&mut gnu_hash, contents.gnu_hash.bucket_count);
        write_u32(&mut gnu_hash, contents.gnu_hash.symbol_offset);
        write_u32(&mut gnu_hash, contents.gnu_hash.bloom_count);
        write_u32(&mut gnu_hash, contents.gnu_hash.bloom_shift);
        for word in &contents.gnu_hash.bloom {
            write_u64(&mut gnu_hash, *word);
        }
        for word in contents
            .gnu_hash
            .buckets
            .iter()
            .chain(&contents.gnu_hash.chains)
        {
            write_u32(&mut gnu_hash, *word);
        }

        let mut versym = Vec::with_capacity(checked_product(
            contents.versym.len(),
            2,
            "GNU symbol-version payload size",
        )?);
        for version in &contents.versym {
            write_u16(&mut versym, *version);
        }

        let verneed_size = contents.verneed.iter().try_fold(0usize, |size, need| {
            let auxiliaries = checked_product(
                need.auxiliaries.len(),
                ELF64_VERSION_AUXILIARY_SIZE,
                "GNU version auxiliary payload size",
            )?;
            size.checked_add(ELF64_VERSION_NEED_SIZE)
                .and_then(|size| size.checked_add(auxiliaries))
                .ok_or_else(|| Diagnostic::error("GNU version requirement payload size overflow"))
        })?;
        let mut verneed = Vec::with_capacity(verneed_size);
        for need in &contents.verneed {
            write_u16(&mut verneed, need.version);
            write_u16(&mut verneed, need.count);
            write_u32(&mut verneed, need.file);
            write_u32(&mut verneed, need.auxiliary_offset);
            write_u32(&mut verneed, need.next_offset);
            for auxiliary in &need.auxiliaries {
                write_u32(&mut verneed, auxiliary.hash);
                write_u16(&mut verneed, auxiliary.flags);
                write_u16(&mut verneed, auxiliary.other);
                write_u32(&mut verneed, auxiliary.name);
                write_u32(&mut verneed, auxiliary.next_offset);
            }
        }

        Ok(ElfDynamicSectionPayloadBytes {
            interpreter: contents.interpreter.clone(),
            dynstr: contents.dynstr.clone(),
            dynsym,
            sysv_hash,
            gnu_hash,
            versym,
            verneed,
        })
    }

    checked::seal_stage! {
        field: plan,
        contents: payloads,
        validated: ValidatedElfDynamicSectionPayloads,
        validate: validate_payloads,
        fingerprint: non_authoritative_payload_compatibility_fingerprint,
        message: "ELF dynamic payload compatibility fingerprint does not replay",
    }

    fn validate_payloads(
        plan: &ValidatedElfDynamicSectionPlan,
        payloads: &ElfDynamicSectionPayloadBytes,
    ) -> Result<(), Diagnostic> {
        let contents = plan.contents();
        require(
            payloads.interpreter == contents.interpreter
                && payloads.interpreter.last() == Some(&0)
                && payloads
                    .interpreter
                    .iter()
                    .filter(|byte| **byte == 0)
                    .count()
                    == 1,
            "serialized PT_INTERP payload does not preserve one exact terminated pathname",
        )?;
        require(
            payloads.dynstr == contents.dynstr
                && payloads.dynstr.first() == Some(&0)
                && payloads.dynstr.last() == Some(&0),
            "serialized dynamic string payload is not exact or NUL framed",
        )?;

        let dynsym = decode_dynsym(&payloads.dynsym, contents.dynsym.len())?;
        require(
            dynsym == contents.dynsym,
            "decoded ELF64 dynamic symbols drifted from the validated plan",
        )?;
        let sysv_hash = decode_sysv_hash(&payloads.sysv_hash)?;
        require(
            sysv_hash == contents.sysv_hash && sysv_hash.chain_count as usize == dynsym.len(),
            "decoded System V hash drifted from the validated dynamic symbols",
        )?;
        let gnu_hash = decode_gnu_hash(&payloads.gnu_hash, dynsym.len())?;
        require(
            gnu_hash == contents.gnu_hash,
            "decoded GNU hash drifted from the validated dynamic symbols",
        )?;
        let versym = decode_versym(&payloads.versym, contents.versym.len())?;
        require(
            versym == contents.versym && versym.len() == dynsym.len(),
            "decoded GNU symbol-version rows drifted from the dynamic symbols",
        )?;
        let verneed = decode_verneed(&payloads.verneed, contents.verneed.len())?;
        require(
            verneed == contents.verneed,
            "decoded GNU version requirements drifted from the validated plan",
        )?;

        for symbol in dynsym.iter().skip(1) {
            name_at(
                &payloads.dynstr,
                symbol.name,
                "ELF dynamic string reference",
            )?;
        }
        for offset in &contents.needed {
            name_at(&payloads.dynstr, *offset, "ELF dynamic string reference")?;
        }
        for need in &verneed {
            name_at(&payloads.dynstr, need.file, "ELF dynamic string reference")?;
            for auxiliary in &need.auxiliaries {
                name_at(
                    &payloads.dynstr,
                    auxiliary.name,
                    "ELF dynamic string reference",
                )?;
            }
        }
        Ok(())
    }

    fn decode_dynsym(
        bytes: &[u8],
        symbol_count: usize,
    ) -> Result<Vec<ElfDynamicSymbol>, Diagnostic> {
        let expected_size = checked_product(
            symbol_count,
            ELF64_SYMBOL_SIZE,
            "decoded ELF64 dynamic symbol payload size",
        )?;
        require(
            bytes.len() == expected_size,
            "ELF64 dynamic symbol payload has a truncated row or trailing bytes",
        )?;
        let mut symbols = Vec::with_capacity(symbol_count);
        for index in 0..symbol_count {
            let offset = checked_product(index, ELF64_SYMBOL_SIZE, "ELF64 symbol row offset")?;
            symbols.push(ElfDynamicSymbol {
                name: read_u32(bytes, offset, "Elf64_Sym.st_name")?,
                info: read_u8(bytes, offset + 4, "Elf64_Sym.st_info")?,
                other: read_u8(bytes, offset + 5, "Elf64_Sym.st_other")?,
                section_index: read_u16(bytes, offset + 6, "Elf64_Sym.st_shndx")?,
                value: read_u64(bytes, offset + 8, "Elf64_Sym.st_value")?,
                size: read_u64(bytes, offset + 16, "Elf64_Sym.st_size")?,
            });
        }
        require(
            symbols.first() == Some(&ElfDynamicSymbol::default()),
            "decoded ELF64 dynamic symbols lack the reserved zero row",
        )?;
        Ok(symbols)
    }

    fn decode_sysv_hash(bytes: &[u8]) -> Result<ElfSysvHash, Diagnostic> {
        let bucket_count = read_u32(bytes, 0, "System V nbucket")?;
        let chain_count = read_u32(bytes, 4, "System V nchain")?;
        require(
            bucket_count > 0 && chain_count > 0,
            "System V hash payload has a zero table count",
        )?;
        let word_count = usize::try_from(bucket_count)
            .ok()
            .and_then(|buckets| buckets.checked_add(chain_count as usize))
            .ok_or_else(|| Diagnostic::error("System V hash decoded word count overflow"))?;
        let expected_size = checked_product(word_count, 4, "System V decoded word byte count")?
            .checked_add(8)
            .ok_or_else(|| Diagnostic::error("System V decoded payload size overflow"))?;
        require(
            bytes.len() == expected_size,
            "System V hash counts do not consume the exact payload bytes",
        )?;

        let mut offset = 8usize;
        let mut buckets = Vec::with_capacity(bucket_count as usize);
        for _ in 0..bucket_count {
            buckets.push(read_u32(bytes, offset, "System V hash bucket")?);
            offset += 4;
        }
        let mut chains = Vec::with_capacity(chain_count as usize);
        for _ in 0..chain_count {
            chains.push(read_u32(bytes, offset, "System V hash chain")?);
            offset += 4;
        }
        require(
            buckets
                .iter()
                .chain(&chains)
                .all(|index| *index < chain_count)
                && chains.first() == Some(&0),
            "System V hash payload contains an out-of-range symbol index",
        )?;
        for bucket in &buckets {
            let mut index = *bucket;
            let mut steps = 0u32;
            while index != 0 {
                index = chains[index as usize];
                steps = steps.saturating_add(1);
                require(
                    steps <= chain_count,
                    "System V hash payload contains a chain cycle",
                )?;
            }
        }
        Ok(ElfSysvHash {
            bucket_count,
            chain_count,
            buckets,
            chains,
        })
    }

    fn decode_gnu_hash(
        bytes: &[u8],
        dynamic_symbol_count: usize,
    ) -> Result<ElfGnuHash, Diagnostic> {
        let bucket_count = read_u32(bytes, 0, "GNU hash nbuckets")?;
        let symbol_offset = read_u32(bytes, 4, "GNU hash symoffset")?;
        let bloom_count = read_u32(bytes, 8, "GNU hash bloom_size")?;
        let bloom_shift = read_u32(bytes, 12, "GNU hash bloom_shift")?;
        require(
            bucket_count > 0 && bloom_count > 0 && symbol_offset > 0,
            "GNU hash payload has a zero table count or symbol offset",
        )?;
        let symbol_offset = symbol_offset as usize;
        require(
            symbol_offset <= dynamic_symbol_count,
            "GNU hash symbol offset exceeds the dynamic symbol count",
        )?;
        let chain_count = dynamic_symbol_count - symbol_offset;
        let bloom_bytes =
            checked_product(bloom_count as usize, 8, "GNU hash decoded bloom byte count")?;
        let bucket_bytes = checked_product(
            bucket_count as usize,
            4,
            "GNU hash decoded bucket byte count",
        )?;
        let chain_bytes = checked_product(chain_count, 4, "GNU hash decoded chain byte count")?;
        let expected_size = checked_sum(16, bloom_bytes, "GNU hash decoded bloom end")?
            .checked_add(bucket_bytes)
            .and_then(|size| size.checked_add(chain_bytes))
            .ok_or_else(|| Diagnostic::error("GNU hash decoded payload size overflow"))?;
        require(
            bytes.len() == expected_size,
            "GNU hash counts do not consume the exact payload bytes",
        )?;

        let mut offset = 16usize;
        let mut bloom = Vec::with_capacity(bloom_count as usize);
        for _ in 0..bloom_count {
            bloom.push(read_u64(bytes, offset, "GNU hash bloom word")?);
            offset += 8;
        }
        let mut buckets = Vec::with_capacity(bucket_count as usize);
        for _ in 0..bucket_count {
            buckets.push(read_u32(bytes, offset, "GNU hash bucket")?);
            offset += 4;
        }
        let mut chains = Vec::with_capacity(chain_count);
        for _ in 0..chain_count {
            chains.push(read_u32(bytes, offset, "GNU hash chain")?);
            offset += 4;
        }
        require(
            buckets.iter().all(|index| {
                *index == 0
                    || ((*index as usize) >= symbol_offset
                        && (*index as usize) < dynamic_symbol_count)
            }),
            "GNU hash bucket contains an out-of-range dynamic symbol index",
        )?;
        for bucket in buckets.iter().copied().filter(|bucket| *bucket != 0) {
            let mut symbol_index = bucket as usize;
            let mut steps = 0usize;
            loop {
                let chain_index = symbol_index - symbol_offset;
                let chain = *chains.get(chain_index).ok_or_else(|| {
                    Diagnostic::error("GNU hash chain exceeds the dynamic symbol domain")
                })?;
                steps += 1;
                require(
                    steps <= chains.len(),
                    "GNU hash chain lacks a bounded terminator",
                )?;
                if chain & 1 != 0 {
                    break;
                }
                symbol_index = symbol_index
                    .checked_add(1)
                    .ok_or_else(|| Diagnostic::error("GNU hash dynamic symbol index overflow"))?;
                require(
                    symbol_index < dynamic_symbol_count,
                    "GNU hash chain lacks a terminator before the symbol-table end",
                )?;
            }
        }
        Ok(ElfGnuHash {
            bucket_count,
            symbol_offset: symbol_offset as u32,
            bloom_count,
            bloom_shift,
            bloom,
            buckets,
            chains,
        })
    }

    fn decode_versym(bytes: &[u8], symbol_count: usize) -> Result<Vec<u16>, Diagnostic> {
        let expected_size =
            checked_product(symbol_count, 2, "decoded GNU symbol-version payload size")?;
        require(
            bytes.len() == expected_size,
            "GNU symbol-version payload has a truncated row or trailing bytes",
        )?;
        (0..symbol_count)
            .map(|index| read_u16(bytes, index * 2, "GNU symbol-version row"))
            .collect()
    }

    fn decode_verneed(bytes: &[u8], need_count: usize) -> Result<Vec<ElfVersionNeed>, Diagnostic> {
        if need_count == 0 {
            require(
                bytes.is_empty(),
                "GNU version requirement payload has rows without a required object",
            )?;
            return Ok(Vec::new());
        }
        let mut needs = Vec::with_capacity(need_count);
        let mut need_offset = 0usize;
        let mut consumed_end = 0usize;
        for need_index in 0..need_count {
            let version = read_u16(bytes, need_offset, "Elf64_Verneed.vn_version")?;
            let count = read_u16(bytes, need_offset + 2, "Elf64_Verneed.vn_cnt")?;
            let file = read_u32(bytes, need_offset + 4, "Elf64_Verneed.vn_file")?;
            let auxiliary_offset = read_u32(bytes, need_offset + 8, "Elf64_Verneed.vn_aux")?;
            let next_offset = read_u32(bytes, need_offset + 12, "Elf64_Verneed.vn_next")?;
            require(
                count > 0 && auxiliary_offset == ELF64_VERSION_NEED_SIZE as u32,
                "GNU version requirement has an empty or noncanonical auxiliary chain",
            )?;
            let canonical_group_size = checked_product(
                count as usize,
                ELF64_VERSION_AUXILIARY_SIZE,
                "GNU version requirement group auxiliary size",
            )?
            .checked_add(ELF64_VERSION_NEED_SIZE)
            .ok_or_else(|| Diagnostic::error("GNU version requirement group size overflow"))?;
            let group_end = checked_sum(
                need_offset,
                canonical_group_size,
                "GNU version requirement group end",
            )?;
            require(
                group_end <= bytes.len(),
                "GNU version requirement count exceeds the remaining payload bytes",
            )?;
            let mut auxiliary_cursor = checked_sum(
                need_offset,
                auxiliary_offset as usize,
                "GNU version auxiliary offset",
            )?;
            let mut auxiliaries = Vec::with_capacity(count as usize);
            for auxiliary_index in 0..count {
                let hash = read_u32(bytes, auxiliary_cursor, "Elf64_Vernaux.vna_hash")?;
                let flags = read_u16(bytes, auxiliary_cursor + 4, "Elf64_Vernaux.vna_flags")?;
                let other = read_u16(bytes, auxiliary_cursor + 6, "Elf64_Vernaux.vna_other")?;
                let name = read_u32(bytes, auxiliary_cursor + 8, "Elf64_Vernaux.vna_name")?;
                let auxiliary_next =
                    read_u32(bytes, auxiliary_cursor + 12, "Elf64_Vernaux.vna_next")?;
                let last_auxiliary = auxiliary_index + 1 == count;
                require(
                    (last_auxiliary && auxiliary_next == 0)
                        || (!last_auxiliary
                            && auxiliary_next == ELF64_VERSION_AUXILIARY_SIZE as u32),
                    "GNU version auxiliary chain has a noncanonical next offset",
                )?;
                auxiliaries.push(ElfVersionNeedAuxiliary {
                    hash,
                    flags,
                    other,
                    name,
                    next_offset: auxiliary_next,
                });
                if !last_auxiliary {
                    auxiliary_cursor = checked_sum(
                        auxiliary_cursor,
                        auxiliary_next as usize,
                        "GNU next version auxiliary offset",
                    )?;
                }
            }
            consumed_end = checked_sum(
                auxiliary_cursor,
                ELF64_VERSION_AUXILIARY_SIZE,
                "GNU version requirement consumed byte count",
            )?;
            let last_need = need_index + 1 == need_count;
            require(
                (last_need && next_offset == 0)
                    || (!last_need && next_offset as usize == canonical_group_size),
                "GNU version requirement chain has a noncanonical next offset",
            )?;
            needs.push(ElfVersionNeed {
                version,
                count,
                file,
                auxiliary_offset,
                next_offset,
                auxiliaries,
            });
            if !last_need {
                need_offset = checked_sum(
                    need_offset,
                    next_offset as usize,
                    "GNU next version requirement offset",
                )?;
            }
        }
        require(
            consumed_end == bytes.len(),
            "GNU version requirement chain does not consume the exact payload bytes",
        )?;
        Ok(needs)
    }

    fn non_authoritative_payload_compatibility_fingerprint(
        plan: &ValidatedElfDynamicSectionPlan,
        payloads: &ElfDynamicSectionPayloadBytes,
    ) -> u64 {
        let mut hash = Fnv1a::new();
        hash.bytes(b"omega.elf-dynamic-section-payloads.v2");
        hash.bytes(b"ELFCLASS64");
        hash.bytes(b"ELFDATA2LSB");
        hash.bytes(
            &plan
                .non_authoritative_content_compatibility_fingerprint()
                .to_le_bytes(),
        );
        for (kind, bytes) in [
            (b".interp".as_slice(), payloads.interpreter.as_slice()),
            (b".dynstr".as_slice(), payloads.dynstr.as_slice()),
            (b".dynsym".as_slice(), payloads.dynsym.as_slice()),
            (b".hash".as_slice(), payloads.sysv_hash.as_slice()),
            (b".gnu.hash".as_slice(), payloads.gnu_hash.as_slice()),
            (b".gnu.version".as_slice(), payloads.versym.as_slice()),
            (b".gnu.version_r".as_slice(), payloads.verneed.as_slice()),
        ] {
            hash.bytes(kind);
            hash.bytes(bytes);
        }
        hash.finish()
    }

    #[cfg(test)]
    mod tests {
        use super::{
            Candidate, ValidatedElfDynamicSectionPlan, decode_dynsym, decode_gnu_hash,
            decode_sysv_hash, decode_verneed, decode_versym, encode_payloads,
            non_authoritative_payload_compatibility_fingerprint, read_u16, read_u32,
            serialize_elf_dynamic_sections, validate_candidate, validate_payloads,
        };
        use crate::bytes::name_at;
        use crate::plan_elf_dynamic_sections;
        use crate::test_support::{SINGLE_SITE_IMPORTS as IMPORTS, SingleSiteImportFixture};
        use target::TargetProfile;

        fn interpreter_path(target: TargetProfile) -> &'static [u8] {
            crate::test_support::import_fixture_interpreter_path(target)
        }

        fn structural_plan(
            target: TargetProfile,
            imports: &[SingleSiteImportFixture],
        ) -> ValidatedElfDynamicSectionPlan {
            structural_plan_with_interpreter(target, imports, interpreter_path(target))
        }

        fn structural_plan_with_interpreter(
            target: TargetProfile,
            imports: &[SingleSiteImportFixture],
            interpreter_path: &[u8],
        ) -> ValidatedElfDynamicSectionPlan {
            let inputs = crate::test_support::single_site_import_inputs(
                target,
                imports,
                "__omega_payload_import_",
                64,
                interpreter_path,
                "valid payload fixture locator",
                "valid payload fixture interpreter",
            );
            plan_elf_dynamic_sections(inputs).expect("valid structural dynamic sections")
        }

        fn candidate(target: TargetProfile) -> Candidate {
            let plan = structural_plan(target, &IMPORTS);
            let payloads = encode_payloads(plan.contents()).expect("encoded payloads");
            let non_authoritative_payload_compatibility_fingerprint =
                non_authoritative_payload_compatibility_fingerprint(&plan, &payloads);
            Candidate {
                plan,
                payloads,
                non_authoritative_payload_compatibility_fingerprint,
            }
        }

        fn words(bytes: &[u8]) -> Vec<u32> {
            bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|word| u32::from_le_bytes(*word))
                .collect()
        }

        #[test]
        fn both_linux_targets_serialize_exact_elf64_lsb_payloads() {
            for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
                let serialized = serialize_elf_dynamic_sections(structural_plan(target, &IMPORTS))
                    .expect("validated serialized payloads");
                let payloads = &serialized.payloads;

                assert_eq!(
                    payloads.interpreter,
                    [interpreter_path(target), &[0]].concat()
                );
                assert_eq!(payloads.dynstr.len(), 64);
                assert_eq!(
                    payloads.dynstr,
                    b"\0OMEGA_1\xfd\0OMEGA_2\0alpha\xfe\0beta\0gamma\0libalpha\xff.so.1\0libbeta.so.2\0"
                );
                assert_eq!(payloads.dynsym.len(), 120);
                assert_eq!(&payloads.dynsym[..24], &[0; 24]);
                assert_eq!(
                    &payloads.dynsym[24..48],
                    &[
                        18, 0, 0, 0, 0x12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    ]
                );
                assert_eq!(payloads.sysv_hash.len(), 44);
                assert_eq!(
                    words(&payloads.sysv_hash),
                    [4, 5, 0, 3, 1, 0, 0, 2, 0, 4, 0]
                );
                assert_eq!(payloads.gnu_hash.len(), 44);
                assert_eq!(
                    words(&payloads.gnu_hash),
                    [
                        1,
                        1,
                        1,
                        5,
                        0x0090_2200,
                        0x0000_0102,
                        1,
                        0xf204_f288,
                        0xf204_f288,
                        0x7c94_89a0,
                        0x0f7d_eae9,
                    ]
                );
                assert_eq!(payloads.versym, [0, 0, 2, 0, 3, 0, 2, 0, 4, 0]);
                assert_eq!(payloads.verneed.len(), 80);
                assert_eq!(read_u16(&payloads.verneed, 0, "version").unwrap(), 1);
                assert_eq!(read_u16(&payloads.verneed, 2, "count").unwrap(), 2);
                assert_eq!(read_u32(&payloads.verneed, 4, "file").unwrap(), 36);
                assert_eq!(read_u32(&payloads.verneed, 8, "aux").unwrap(), 16);
                assert_eq!(read_u32(&payloads.verneed, 12, "next").unwrap(), 48);
                assert_eq!(read_u16(&payloads.verneed, 22, "other").unwrap(), 2);
                assert_eq!(read_u32(&payloads.verneed, 24, "name").unwrap(), 1);
                assert_eq!(read_u32(&payloads.verneed, 28, "next").unwrap(), 16);
                assert_eq!(read_u16(&payloads.verneed, 38, "other").unwrap(), 3);
                assert_eq!(read_u32(&payloads.verneed, 44, "next").unwrap(), 0);
                assert_eq!(read_u32(&payloads.verneed, 52, "file").unwrap(), 51);
                assert_eq!(read_u16(&payloads.verneed, 70, "other").unwrap(), 4);
                assert_eq!(serialized.dynamic_string_byte_count(), 64);
                assert_eq!(serialized.dynamic_symbol_byte_count(), 120);
                assert_eq!(serialized.system_v_hash_byte_count(), 44);
                assert_eq!(serialized.gnu_hash_byte_count(), 44);
                assert_eq!(serialized.symbol_version_byte_count(), 10);
                assert_eq!(serialized.version_requirement_byte_count(), 80);
                assert_ne!(
                    serialized.non_authoritative_payload_compatibility_fingerprint(),
                    0
                );
                validate_payloads(serialized.plan(), payloads).expect("independent byte replay");
            }
        }

        #[test]
        fn serialized_payloads_and_identity_ignore_import_insertion_order() {
            let forward =
                serialize_elf_dynamic_sections(structural_plan(TargetProfile::LinuxX64, &IMPORTS))
                    .expect("forward payloads");
            let reversed = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
            let reverse =
                serialize_elf_dynamic_sections(structural_plan(TargetProfile::LinuxX64, &reversed))
                    .expect("reverse payloads");

            assert_eq!(forward.payloads, reverse.payloads);
            assert_eq!(
                forward.non_authoritative_payload_compatibility_fingerprint(),
                reverse.non_authoritative_payload_compatibility_fingerprint()
            );
        }

        #[test]
        fn payload_identity_binds_profile_and_exact_serialized_coordinates() {
            let baseline =
                serialize_elf_dynamic_sections(structural_plan(TargetProfile::LinuxX64, &IMPORTS))
                    .expect("baseline payloads")
                    .non_authoritative_payload_compatibility_fingerprint();
            let changed_profile = serialize_elf_dynamic_sections(structural_plan(
                TargetProfile::LinuxArm64,
                &IMPORTS,
            ))
            .expect("changed-profile payloads")
            .non_authoritative_payload_compatibility_fingerprint();
            let changed_interpreter =
                serialize_elf_dynamic_sections(structural_plan_with_interpreter(
                    TargetProfile::LinuxX64,
                    &IMPORTS,
                    b"/another/ld-linux-x86-64.so.2",
                ))
                .expect("changed-interpreter payloads")
                .non_authoritative_payload_compatibility_fingerprint();
            let mut changed_imports = IMPORTS;
            changed_imports[0] = SingleSiteImportFixture {
                object: b"libalpha\xff.so.1",
                symbol: b"changed_alpha\xfe",
                version: b"OMEGA_1\xfd",
            };
            let changed_coordinate = serialize_elf_dynamic_sections(structural_plan(
                TargetProfile::LinuxX64,
                &changed_imports,
            ))
            .expect("changed-coordinate payloads")
            .non_authoritative_payload_compatibility_fingerprint();

            assert_ne!(baseline, changed_profile);
            assert_ne!(baseline, changed_interpreter);
            assert_ne!(baseline, changed_coordinate);
        }

        #[test]
        fn independent_decoder_rejects_every_payload_and_identity_corruption() {
            let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
                Box::new(|candidate| {
                    candidate.payloads.interpreter.pop();
                }),
                Box::new(|candidate| candidate.payloads.interpreter.push(0)),
                Box::new(|candidate| candidate.payloads.dynstr[0] = 1),
                Box::new(|candidate| {
                    candidate.payloads.dynstr.pop();
                }),
                Box::new(|candidate| {
                    candidate.payloads.dynsym.pop();
                }),
                Box::new(|candidate| candidate.payloads.dynsym.push(0)),
                Box::new(|candidate| candidate.payloads.dynsym[24..28].reverse()),
                Box::new(|candidate| candidate.payloads.dynsym[24] = 0),
                Box::new(|candidate| {
                    candidate.payloads.sysv_hash.pop();
                }),
                Box::new(|candidate| candidate.payloads.sysv_hash.push(0)),
                Box::new(|candidate| candidate.payloads.sysv_hash[..4].fill(0)),
                Box::new(|candidate| candidate.payloads.sysv_hash[4..8].fill(0xff)),
                Box::new(|candidate| candidate.payloads.sysv_hash[8..12].fill(0xff)),
                Box::new(|candidate| {
                    candidate.payloads.sysv_hash[28..32].copy_from_slice(&1u32.to_le_bytes())
                }),
                Box::new(|candidate| {
                    candidate.payloads.gnu_hash.pop();
                }),
                Box::new(|candidate| candidate.payloads.gnu_hash.push(0)),
                Box::new(|candidate| candidate.payloads.gnu_hash[..4].fill(0)),
                Box::new(|candidate| candidate.payloads.gnu_hash[4..8].fill(0)),
                Box::new(|candidate| candidate.payloads.gnu_hash[8..12].fill(0xff)),
                Box::new(|candidate| candidate.payloads.gnu_hash[12..16].fill(0xff)),
                Box::new(|candidate| candidate.payloads.gnu_hash[16] ^= 1),
                Box::new(|candidate| candidate.payloads.gnu_hash[24..28].fill(0xff)),
                Box::new(|candidate| candidate.payloads.gnu_hash[28] ^= 2),
                Box::new(|candidate| candidate.payloads.gnu_hash[40] &= !1),
                Box::new(|candidate| {
                    candidate.payloads.versym.pop();
                }),
                Box::new(|candidate| candidate.payloads.versym.push(0)),
                Box::new(|candidate| candidate.payloads.versym[2] = 1),
                Box::new(|candidate| {
                    candidate.payloads.verneed.pop();
                }),
                Box::new(|candidate| candidate.payloads.verneed.push(0)),
                Box::new(|candidate| candidate.payloads.verneed[2..4].fill(0xff)),
                Box::new(|candidate| candidate.payloads.verneed[8..12].fill(0)),
                Box::new(|candidate| candidate.payloads.verneed[12..16].fill(0)),
                Box::new(|candidate| candidate.payloads.verneed[28..32].fill(0)),
                Box::new(|candidate| {
                    candidate.non_authoritative_payload_compatibility_fingerprint ^= 1
                }),
            ];

            for corrupt in corruptions {
                let mut candidate = candidate(TargetProfile::LinuxX64);
                corrupt(&mut candidate);
                let error = validate_candidate(candidate)
                    .expect_err("corrupt bytes must reject before sealing payloads");
                assert_eq!(
                    error.candidate.plan.inputs().interpreter().target(),
                    TargetProfile::LinuxX64,
                    "decoder rejection retains exact structural-plan custody",
                );
            }
        }

        #[test]
        fn adversarial_lengths_counts_indexes_and_offsets_reject_without_panicking() {
            for bytes in [Vec::new(), vec![0; 7], vec![0xff; 8], vec![0; 12]] {
                assert!(decode_sysv_hash(&bytes).is_err());
            }
            let out_of_range_hash = [1u32, 2, u32::MAX, 0, 0].map(u32::to_le_bytes).concat();
            assert!(decode_sysv_hash(&out_of_range_hash).is_err());
            let cyclic_hash = [1u32, 3, 1, 0, 1, 0].map(u32::to_le_bytes).concat();
            assert!(decode_sysv_hash(&cyclic_hash).is_err());

            for bytes in [
                Vec::new(),
                vec![0; 15],
                vec![0xff; 16],
                vec![0; 20],
                vec![0; 44],
            ] {
                assert!(decode_gnu_hash(&bytes, 5).is_err());
            }
            let mut out_of_range_gnu = candidate(TargetProfile::LinuxX64).payloads.gnu_hash;
            out_of_range_gnu[24..28].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(decode_gnu_hash(&out_of_range_gnu, 5).is_err());
            let mut unterminated_gnu = candidate(TargetProfile::LinuxX64).payloads.gnu_hash;
            unterminated_gnu[40] &= !1;
            assert!(decode_gnu_hash(&unterminated_gnu, 5).is_err());

            for bytes in [Vec::new(), vec![0; 15], vec![0xff; 16], vec![0; 32]] {
                assert!(decode_verneed(&bytes, 1).is_err());
            }
            let mut cyclic_auxiliary = candidate(TargetProfile::LinuxX64).payloads.verneed;
            cyclic_auxiliary[28..32].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(decode_verneed(&cyclic_auxiliary, 2).is_err());

            assert!(decode_dynsym(&[0; 23], 1).is_err());
            assert!(decode_versym(&[0], 1).is_err());
            assert!(name_at(&[0], u32::MAX, "ELF dynamic string reference").is_err());
        }
    }
}

pub(crate) mod dynamic_section_descriptors {
    use super::super::checked::{self, CandidateValidationError};
    use crate::bytes::{name_at, require_name};
    use crate::dynamic_executable::checked::{read_u32, require};
    use crate::dynamic_executable::import_sections::dynamic_section_bytes::ValidatedElfDynamicSectionPayloads;
    use diagnostics::Diagnostic;

    use crate::constants::SHF_ALLOC;
    const SHT_PROGBITS: u32 = 1;
    const SHT_STRTAB: u32 = 3;
    const SHT_HASH: u32 = 5;
    const SHT_DYNSYM: u32 = 11;
    const SHT_GNU_HASH: u32 = 0x6fff_fff6;
    const SHT_GNU_VERNEED: u32 = 0x6fff_fffe;
    const SHT_GNU_VERSYM: u32 = 0x6fff_ffff;

    const INTERPRETER_NAME_OFFSET: u32 = 1;
    const DYNAMIC_STRING_NAME_OFFSET: u32 = 9;
    const DYNAMIC_SYMBOL_NAME_OFFSET: u32 = 17;
    const SYSTEM_V_HASH_NAME_OFFSET: u32 = 25;
    const GNU_SYMBOL_VERSION_NAME_OFFSET: u32 = 31;
    const GNU_VERSION_REQUIREMENT_NAME_OFFSET: u32 = 44;
    const SECTION_NAME_TABLE_NAME_OFFSET: u32 = 59;
    const GNU_HASH_NAME_OFFSET: u32 = 69;

    const SECTION_NAME_TABLE_SEED: &[u8] =
        b"\0.interp\0.dynstr\0.dynsym\0.hash\0.gnu.version\0.gnu.version_r\0.shstrtab\0.gnu.hash\0";
    const DYNAMIC_SECTION_KINDS: [ElfDynamicSectionKind; 7] = [
        ElfDynamicSectionKind::Interpreter,
        ElfDynamicSectionKind::DynamicString,
        ElfDynamicSectionKind::DynamicSymbol,
        ElfDynamicSectionKind::SystemVHash,
        ElfDynamicSectionKind::GnuSymbolVersion,
        ElfDynamicSectionKind::GnuVersionRequirement,
        ElfDynamicSectionKind::GnuHash,
    ];

    #[derive(Debug)]
    #[must_use = "validated ELF section descriptors retain the exact payload carrier"]
    pub struct ValidatedElfDynamicSectionDescriptorPlan {
        payloads: ValidatedElfDynamicSectionPayloads,
        contents: ElfDynamicSectionDescriptorContents,
        non_authoritative_descriptor_compatibility_fingerprint: u64,
    }

    impl ValidatedElfDynamicSectionDescriptorPlan {
        pub const fn payloads(&self) -> &ValidatedElfDynamicSectionPayloads {
            &self.payloads
        }

        pub fn descriptor_count(&self) -> usize {
            self.contents.descriptors.len()
        }

        #[cfg(test)]
        pub(crate) fn section_name_seed_byte_count(&self) -> usize {
            self.contents.section_name_table_seed.len()
        }

        pub(crate) fn section_name_table_seed(&self) -> &[u8] {
            &self.contents.section_name_table_seed
        }

        pub const fn non_authoritative_descriptor_compatibility_fingerprint(&self) -> u64 {
            self.non_authoritative_descriptor_compatibility_fingerprint
        }

        pub(crate) const fn contents(&self) -> &ElfDynamicSectionDescriptorContents {
            &self.contents
        }

        pub(crate) fn into_parts(
            self,
        ) -> (
            ValidatedElfDynamicSectionPayloads,
            ElfDynamicSectionDescriptorContents,
        ) {
            (self.payloads, self.contents)
        }
    }

    #[derive(Debug)]
    #[must_use = "ELF descriptor rejection retains the validated payload carrier"]
    pub struct ElfDynamicSectionDescriptorPlanningError {
        payloads: ValidatedElfDynamicSectionPayloads,
        diagnostic: Diagnostic,
    }

    image::rejection_carrier!(
        ElfDynamicSectionDescriptorPlanningError;
        payloads: ValidatedElfDynamicSectionPayloads,
    );

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ElfDynamicSectionDescriptorContents {
        pub(crate) section_name_table_seed: Vec<u8>,
        pub(crate) descriptors: Vec<ElfAddressFreeSectionDescriptor>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    pub(crate) enum ElfDynamicSectionKind {
        Interpreter = 1,
        DynamicString = 2,
        DynamicSymbol = 3,
        SystemVHash = 4,
        GnuSymbolVersion = 5,
        GnuVersionRequirement = 6,
        GnuHash = 7,
    }

    impl ElfDynamicSectionKind {
        const fn name(self) -> &'static [u8] {
            match self {
                Self::Interpreter => b".interp",
                Self::DynamicString => b".dynstr",
                Self::DynamicSymbol => b".dynsym",
                Self::SystemVHash => b".hash",
                Self::GnuSymbolVersion => b".gnu.version",
                Self::GnuVersionRequirement => b".gnu.version_r",
                Self::GnuHash => b".gnu.hash",
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ElfAddressFreeSectionDescriptor {
        pub(crate) kind: ElfDynamicSectionKind,
        pub(crate) name_offset: u32,
        pub(crate) section_type: u32,
        pub(crate) flags: u64,
        pub(crate) payload_size: u64,
        pub(crate) alignment: u64,
        pub(crate) entry_size: u64,
        pub(crate) link: Option<ElfDynamicSectionKind>,
        pub(crate) info: u32,
    }

    struct Candidate {
        payloads: ValidatedElfDynamicSectionPayloads,
        contents: ElfDynamicSectionDescriptorContents,
        non_authoritative_descriptor_compatibility_fingerprint: u64,
    }

    pub(crate) fn plan_elf_dynamic_section_descriptors(
        payloads: ValidatedElfDynamicSectionPayloads,
    ) -> Result<
        ValidatedElfDynamicSectionDescriptorPlan,
        Box<ElfDynamicSectionDescriptorPlanningError>,
    > {
        checked::plan_stage!(
            plan: payloads,
            derive: derive_contents,
            contents: contents,
            fingerprint: non_authoritative_descriptor_compatibility_fingerprint,
            error: ElfDynamicSectionDescriptorPlanningError,
        )
    }

    fn derive_contents(
        payloads: &ValidatedElfDynamicSectionPayloads,
    ) -> Result<ElfDynamicSectionDescriptorContents, Diagnostic> {
        let bytes = payloads.payloads();
        let required_version_object_count =
            u32::try_from(payloads.plan().required_version_object_count()).map_err(|_| {
                Diagnostic::error("ELF version-need section info exceeds Elf64_Word")
            })?;
        let descriptors = vec![
            descriptor(
                ElfDynamicSectionKind::Interpreter,
                INTERPRETER_NAME_OFFSET,
                SHT_PROGBITS,
                SHF_ALLOC,
                bytes.interpreter.len(),
                1,
                0,
                None,
                0,
            )?,
            descriptor(
                ElfDynamicSectionKind::DynamicString,
                DYNAMIC_STRING_NAME_OFFSET,
                SHT_STRTAB,
                SHF_ALLOC,
                bytes.dynstr.len(),
                1,
                0,
                None,
                0,
            )?,
            descriptor(
                ElfDynamicSectionKind::DynamicSymbol,
                DYNAMIC_SYMBOL_NAME_OFFSET,
                SHT_DYNSYM,
                SHF_ALLOC,
                bytes.dynsym.len(),
                8,
                24,
                Some(ElfDynamicSectionKind::DynamicString),
                1,
            )?,
            descriptor(
                ElfDynamicSectionKind::SystemVHash,
                SYSTEM_V_HASH_NAME_OFFSET,
                SHT_HASH,
                SHF_ALLOC,
                bytes.sysv_hash.len(),
                4,
                4,
                Some(ElfDynamicSectionKind::DynamicSymbol),
                0,
            )?,
            descriptor(
                ElfDynamicSectionKind::GnuSymbolVersion,
                GNU_SYMBOL_VERSION_NAME_OFFSET,
                SHT_GNU_VERSYM,
                SHF_ALLOC,
                bytes.versym.len(),
                2,
                2,
                Some(ElfDynamicSectionKind::DynamicSymbol),
                0,
            )?,
            descriptor(
                ElfDynamicSectionKind::GnuVersionRequirement,
                GNU_VERSION_REQUIREMENT_NAME_OFFSET,
                SHT_GNU_VERNEED,
                SHF_ALLOC,
                bytes.verneed.len(),
                4,
                0,
                Some(ElfDynamicSectionKind::DynamicString),
                required_version_object_count,
            )?,
            descriptor(
                ElfDynamicSectionKind::GnuHash,
                GNU_HASH_NAME_OFFSET,
                SHT_GNU_HASH,
                SHF_ALLOC,
                bytes.gnu_hash.len(),
                8,
                0,
                Some(ElfDynamicSectionKind::DynamicSymbol),
                0,
            )?,
        ];
        Ok(ElfDynamicSectionDescriptorContents {
            section_name_table_seed: SECTION_NAME_TABLE_SEED.to_vec(),
            descriptors,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn descriptor(
        kind: ElfDynamicSectionKind,
        name_offset: u32,
        section_type: u32,
        flags: u64,
        payload_size: usize,
        alignment: u64,
        entry_size: u64,
        link: Option<ElfDynamicSectionKind>,
        info: u32,
    ) -> Result<ElfAddressFreeSectionDescriptor, Diagnostic> {
        Ok(ElfAddressFreeSectionDescriptor {
            kind,
            name_offset,
            section_type,
            flags,
            payload_size: u64::try_from(payload_size)
                .map_err(|_| Diagnostic::error("ELF section payload size exceeds Elf64_Xword"))?,
            alignment,
            entry_size,
            link,
            info,
        })
    }

    checked::seal_stage! {
        field: payloads,
        contents: contents,
        validated: ValidatedElfDynamicSectionDescriptorPlan,
        validate: validate_contents,
        fingerprint: non_authoritative_descriptor_compatibility_fingerprint,
        message: "ELF dynamic descriptor compatibility fingerprint does not replay",
    }

    fn validate_contents(
        payloads: &ValidatedElfDynamicSectionPayloads,
        contents: &ElfDynamicSectionDescriptorContents,
    ) -> Result<(), Diagnostic> {
        require(
            contents.section_name_table_seed == SECTION_NAME_TABLE_SEED
                && contents.section_name_table_seed.first() == Some(&0)
                && contents.section_name_table_seed.last() == Some(&0),
            "ELF section-name seed is not the exact append-only canonical prefix",
        )?;
        require(
            name_at(
                &contents.section_name_table_seed,
                SECTION_NAME_TABLE_NAME_OFFSET,
                "section-name seed",
            )? == b".shstrtab"
                && contents
                    .descriptors
                    .iter()
                    .all(|descriptor| descriptor.name_offset != SECTION_NAME_TABLE_NAME_OFFSET),
            "ELF section-name seed does not retain an unclaimed future .shstrtab name",
        )?;
        require(
            contents.descriptors.len() == DYNAMIC_SECTION_KINDS.len(),
            "ELF dynamic descriptor plan does not contain exactly seven rows",
        )?;

        for (index, expected_kind) in DYNAMIC_SECTION_KINDS.iter().enumerate() {
            let row = contents.descriptors.get(index).ok_or_else(|| {
                Diagnostic::error("ELF dynamic descriptor row is missing from canonical order")
            })?;
            require(
                row.kind == *expected_kind
                    && contents
                        .descriptors
                        .iter()
                        .filter(|candidate| candidate.kind == *expected_kind)
                        .count()
                        == 1,
                "ELF dynamic descriptor kinds are missing, duplicated, or reordered",
            )?;
            validate_name(&contents.section_name_table_seed, row)?;
            require(
                row.alignment.is_power_of_two(),
                "ELF dynamic descriptor alignment is not a positive power of two",
            )?;
            if row.entry_size != 0 {
                require(
                    row.payload_size % row.entry_size == 0,
                    "ELF dynamic descriptor payload is not divisible by its entry size",
                )?;
            }
            if let Some(link) = row.link {
                require(
                    contents
                        .descriptors
                        .iter()
                        .filter(|candidate| candidate.kind == link)
                        .count()
                        == 1,
                    "ELF dynamic descriptor semantic link does not resolve exactly once",
                )?;
            }
            validate_row(payloads, row)?;
        }
        validate_cross_payloads(payloads, contents)
    }

    fn validate_name(
        section_names: &[u8],
        row: &ElfAddressFreeSectionDescriptor,
    ) -> Result<(), Diagnostic> {
        require_name(
            section_names,
            row.name_offset,
            row.kind.name(),
            "ELF descriptor name offset",
        )
    }

    fn validate_row(
        payloads: &ValidatedElfDynamicSectionPayloads,
        row: &ElfAddressFreeSectionDescriptor,
    ) -> Result<(), Diagnostic> {
        let bytes = payloads.payloads();
        let required_version_object_count =
            u32::try_from(payloads.plan().required_version_object_count()).map_err(|_| {
                Diagnostic::error("validated version-need object count exceeds Elf64_Word")
            })?;
        let expected = match row.kind {
            ElfDynamicSectionKind::Interpreter => (
                INTERPRETER_NAME_OFFSET,
                SHT_PROGBITS,
                bytes.interpreter.len(),
                1,
                0,
                None,
                0,
            ),
            ElfDynamicSectionKind::DynamicString => (
                DYNAMIC_STRING_NAME_OFFSET,
                SHT_STRTAB,
                bytes.dynstr.len(),
                1,
                0,
                None,
                0,
            ),
            ElfDynamicSectionKind::DynamicSymbol => (
                DYNAMIC_SYMBOL_NAME_OFFSET,
                SHT_DYNSYM,
                bytes.dynsym.len(),
                8,
                24,
                Some(ElfDynamicSectionKind::DynamicString),
                1,
            ),
            ElfDynamicSectionKind::SystemVHash => (
                SYSTEM_V_HASH_NAME_OFFSET,
                SHT_HASH,
                bytes.sysv_hash.len(),
                4,
                4,
                Some(ElfDynamicSectionKind::DynamicSymbol),
                0,
            ),
            ElfDynamicSectionKind::GnuSymbolVersion => (
                GNU_SYMBOL_VERSION_NAME_OFFSET,
                SHT_GNU_VERSYM,
                bytes.versym.len(),
                2,
                2,
                Some(ElfDynamicSectionKind::DynamicSymbol),
                0,
            ),
            ElfDynamicSectionKind::GnuVersionRequirement => (
                GNU_VERSION_REQUIREMENT_NAME_OFFSET,
                SHT_GNU_VERNEED,
                bytes.verneed.len(),
                4,
                0,
                Some(ElfDynamicSectionKind::DynamicString),
                required_version_object_count,
            ),
            ElfDynamicSectionKind::GnuHash => (
                GNU_HASH_NAME_OFFSET,
                SHT_GNU_HASH,
                bytes.gnu_hash.len(),
                8,
                0,
                Some(ElfDynamicSectionKind::DynamicSymbol),
                0,
            ),
        };
        require(
            row.name_offset == expected.0
                && row.section_type == expected.1
                && row.flags == SHF_ALLOC
                && row.payload_size == expected.2 as u64
                && row.alignment == expected.3
                && row.entry_size == expected.4
                && row.link == expected.5
                && row.info == expected.6,
            "ELF dynamic descriptor metadata drifted from its exact semantic kind",
        )
    }

    fn validate_cross_payloads(
        payloads: &ValidatedElfDynamicSectionPayloads,
        contents: &ElfDynamicSectionDescriptorContents,
    ) -> Result<(), Diagnostic> {
        let bytes = payloads.payloads();
        require(
            bytes.dynsym.len().is_multiple_of(24)
                && bytes.versym.len().is_multiple_of(2)
                && bytes.dynsym.len() / 24 == bytes.versym.len() / 2,
            "ELF dynamic symbol and GNU symbol-version descriptor counts diverge",
        )?;
        let chain_count = read_u32(&bytes.sysv_hash, 4, "System V nchain")? as usize;
        require(
            chain_count == bytes.dynsym.len() / 24,
            "ELF System V hash nchain does not match descriptor dynamic symbols",
        )?;
        let version_requirement = contents
            .descriptors
            .iter()
            .find(|descriptor| descriptor.kind == ElfDynamicSectionKind::GnuVersionRequirement)
            .ok_or_else(|| Diagnostic::error("ELF GNU version requirement descriptor is absent"))?;
        require(
            version_requirement.info as usize == payloads.plan().required_version_object_count(),
            "ELF GNU version requirement sh_info does not match the Verneed count",
        )
    }

    fn non_authoritative_descriptor_compatibility_fingerprint(
        payloads: &ValidatedElfDynamicSectionPayloads,
        contents: &ElfDynamicSectionDescriptorContents,
    ) -> u64 {
        let mut hash = checked::stage_fingerprint(
            b"omega.elf-dynamic-section-descriptors.v2",
            payloads.non_authoritative_payload_compatibility_fingerprint(),
        );
        hash.bytes(&contents.section_name_table_seed);
        for row in &contents.descriptors {
            hash.byte(row.kind as u8);
            hash.bytes(&row.name_offset.to_le_bytes());
            hash.bytes(&row.section_type.to_le_bytes());
            hash.bytes(&row.flags.to_le_bytes());
            hash.bytes(&row.payload_size.to_le_bytes());
            hash.bytes(&row.alignment.to_le_bytes());
            hash.bytes(&row.entry_size.to_le_bytes());
            hash.byte(row.link.map_or(0, |kind| kind as u8));
            hash.bytes(&row.info.to_le_bytes());
        }
        hash.finish()
    }

    #[cfg(test)]
    mod tests {
        use super::{
            Candidate, ElfAddressFreeSectionDescriptor, ElfDynamicSectionDescriptorContents,
            ElfDynamicSectionKind, SECTION_NAME_TABLE_NAME_OFFSET, SECTION_NAME_TABLE_SEED,
            SHF_ALLOC, SHT_DYNSYM, SHT_GNU_HASH, SHT_GNU_VERNEED, SHT_GNU_VERSYM, SHT_HASH,
            SHT_PROGBITS, SHT_STRTAB, ValidatedElfDynamicSectionPayloads, derive_contents,
            non_authoritative_descriptor_compatibility_fingerprint,
            plan_elf_dynamic_section_descriptors, read_u32, validate_candidate, validate_contents,
        };
        use crate::bytes::name_at;
        use crate::dynamic_executable::interpreter_path;
        use crate::test_support::SingleSiteImportFixture;
        use crate::{plan_elf_dynamic_sections, serialize_elf_dynamic_sections};
        use target::TargetProfile;

        const IMPORTS: [SingleSiteImportFixture; 3] = [
            SingleSiteImportFixture {
                object: b"liba\xff.so",
                symbol: b"alpha\xfe",
                version: b"V1\xfd",
            },
            SingleSiteImportFixture {
                object: b"liba\xff.so",
                symbol: b"beta",
                version: b"V2",
            },
            SingleSiteImportFixture {
                object: b"libb.so",
                symbol: b"gamma",
                version: b"V1\xfd",
            },
        ];

        fn payloads(
            target: TargetProfile,
            imports: &[SingleSiteImportFixture],
        ) -> ValidatedElfDynamicSectionPayloads {
            let inputs = crate::test_support::single_site_import_inputs(
                target,
                imports,
                "__omega_descriptor_import_",
                32,
                interpreter_path(target),
                "valid descriptor fixture locator",
                "valid descriptor fixture interpreter",
            );
            let sections = plan_elf_dynamic_sections(inputs).expect("valid dynamic sections");
            serialize_elf_dynamic_sections(sections).expect("valid serialized payloads")
        }

        fn candidate(target: TargetProfile) -> Candidate {
            let payloads = payloads(target, &IMPORTS);
            let contents = derive_contents(&payloads).expect("derived descriptors");
            let non_authoritative_descriptor_compatibility_fingerprint =
                non_authoritative_descriptor_compatibility_fingerprint(&payloads, &contents);
            Candidate {
                payloads,
                contents,
                non_authoritative_descriptor_compatibility_fingerprint,
            }
        }

        fn row(
            contents: &ElfDynamicSectionDescriptorContents,
            kind: ElfDynamicSectionKind,
        ) -> &ElfAddressFreeSectionDescriptor {
            contents
                .descriptors
                .iter()
                .find(|descriptor| descriptor.kind == kind)
                .expect("one exact descriptor kind")
        }

        #[test]
        fn both_linux_targets_plan_exact_address_free_descriptor_metadata() {
            for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
                let plan = plan_elf_dynamic_section_descriptors(payloads(target, &IMPORTS))
                    .expect("validated descriptor plan");
                let contents = &plan.contents;
                assert_eq!(contents.section_name_table_seed, SECTION_NAME_TABLE_SEED);
                assert_eq!(plan.descriptor_count(), 7);
                assert_eq!(plan.section_name_seed_byte_count(), 79);
                assert_eq!(
                    contents
                        .descriptors
                        .iter()
                        .map(|descriptor| descriptor.name_offset)
                        .collect::<Vec<_>>(),
                    [1, 9, 17, 25, 31, 44, 69],
                );
                assert_eq!(
                    contents
                        .descriptors
                        .iter()
                        .map(|descriptor| descriptor.payload_size)
                        .collect::<Vec<_>>(),
                    [
                        plan.payloads().interpreter_byte_count() as u64,
                        43,
                        96,
                        36,
                        8,
                        80,
                        40,
                    ],
                );

                assert_eq!(
                    *row(contents, ElfDynamicSectionKind::Interpreter),
                    ElfAddressFreeSectionDescriptor {
                        kind: ElfDynamicSectionKind::Interpreter,
                        name_offset: 1,
                        section_type: SHT_PROGBITS,
                        flags: SHF_ALLOC,
                        payload_size: plan.payloads().interpreter_byte_count() as u64,
                        alignment: 1,
                        entry_size: 0,
                        link: None,
                        info: 0,
                    }
                );
                assert_eq!(
                    *row(contents, ElfDynamicSectionKind::DynamicSymbol),
                    ElfAddressFreeSectionDescriptor {
                        kind: ElfDynamicSectionKind::DynamicSymbol,
                        name_offset: 17,
                        section_type: SHT_DYNSYM,
                        flags: SHF_ALLOC,
                        payload_size: 96,
                        alignment: 8,
                        entry_size: 24,
                        link: Some(ElfDynamicSectionKind::DynamicString),
                        info: 1,
                    }
                );
                assert_eq!(
                    *row(contents, ElfDynamicSectionKind::SystemVHash),
                    ElfAddressFreeSectionDescriptor {
                        kind: ElfDynamicSectionKind::SystemVHash,
                        name_offset: 25,
                        section_type: SHT_HASH,
                        flags: SHF_ALLOC,
                        payload_size: 36,
                        alignment: 4,
                        entry_size: 4,
                        link: Some(ElfDynamicSectionKind::DynamicSymbol),
                        info: 0,
                    }
                );
                assert_eq!(
                    *row(contents, ElfDynamicSectionKind::GnuSymbolVersion),
                    ElfAddressFreeSectionDescriptor {
                        kind: ElfDynamicSectionKind::GnuSymbolVersion,
                        name_offset: 31,
                        section_type: SHT_GNU_VERSYM,
                        flags: SHF_ALLOC,
                        payload_size: 8,
                        alignment: 2,
                        entry_size: 2,
                        link: Some(ElfDynamicSectionKind::DynamicSymbol),
                        info: 0,
                    }
                );
                assert_eq!(
                    *row(contents, ElfDynamicSectionKind::GnuVersionRequirement),
                    ElfAddressFreeSectionDescriptor {
                        kind: ElfDynamicSectionKind::GnuVersionRequirement,
                        name_offset: 44,
                        section_type: SHT_GNU_VERNEED,
                        flags: SHF_ALLOC,
                        payload_size: 80,
                        alignment: 4,
                        entry_size: 0,
                        link: Some(ElfDynamicSectionKind::DynamicString),
                        info: 2,
                    }
                );
                assert_eq!(
                    *row(contents, ElfDynamicSectionKind::GnuHash),
                    ElfAddressFreeSectionDescriptor {
                        kind: ElfDynamicSectionKind::GnuHash,
                        name_offset: 69,
                        section_type: SHT_GNU_HASH,
                        flags: SHF_ALLOC,
                        payload_size: 40,
                        alignment: 8,
                        entry_size: 0,
                        link: Some(ElfDynamicSectionKind::DynamicSymbol),
                        info: 0,
                    }
                );
                assert_ne!(
                    plan.non_authoritative_descriptor_compatibility_fingerprint(),
                    0
                );
                validate_contents(plan.payloads(), contents)
                    .expect("independent descriptor replay");
            }
        }

        #[test]
        fn semantic_links_and_append_only_name_seed_preserve_exact_names() {
            let plan =
                plan_elf_dynamic_section_descriptors(payloads(TargetProfile::LinuxX64, &IMPORTS))
                    .expect("validated descriptor plan");
            let mut extended = plan.contents.section_name_table_seed.clone();
            let expected = plan
                .contents
                .descriptors
                .iter()
                .map(|descriptor| {
                    (
                        descriptor.kind,
                        name_at(&extended, descriptor.name_offset, "section-name seed")
                            .unwrap()
                            .to_vec(),
                    )
                })
                .collect::<Vec<_>>();
            extended.extend(b".dynamic\0.rela.dyn\0");

            for (kind, name) in expected {
                let descriptor = row(&plan.contents, kind);
                assert_eq!(
                    name_at(&extended, descriptor.name_offset, "section-name seed").unwrap(),
                    name
                );
            }
            assert_eq!(
                name_at(
                    &extended,
                    SECTION_NAME_TABLE_NAME_OFFSET,
                    "section-name seed",
                )
                .unwrap(),
                b".shstrtab"
            );
            assert_eq!(
                row(&plan.contents, ElfDynamicSectionKind::DynamicSymbol).link,
                Some(ElfDynamicSectionKind::DynamicString)
            );
            assert_eq!(
                row(&plan.contents, ElfDynamicSectionKind::SystemVHash).link,
                Some(ElfDynamicSectionKind::DynamicSymbol)
            );
            assert_eq!(
                row(&plan.contents, ElfDynamicSectionKind::GnuHash).link,
                Some(ElfDynamicSectionKind::DynamicSymbol)
            );
        }

        #[test]
        fn descriptor_identity_ignores_import_insertion_order_and_binds_profile() {
            let forward =
                plan_elf_dynamic_section_descriptors(payloads(TargetProfile::LinuxX64, &IMPORTS))
                    .expect("forward descriptor plan");
            let reversed = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
            let reverse =
                plan_elf_dynamic_section_descriptors(payloads(TargetProfile::LinuxX64, &reversed))
                    .expect("reverse descriptor plan");
            let arm =
                plan_elf_dynamic_section_descriptors(payloads(TargetProfile::LinuxArm64, &IMPORTS))
                    .expect("arm descriptor plan");

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
        fn independent_validation_rejects_every_name_metadata_link_and_identity_corruption() {
            let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
                Box::new(|candidate| candidate.contents.section_name_table_seed[0] = 1),
                Box::new(|candidate| {
                    candidate.contents.section_name_table_seed.pop();
                }),
                Box::new(|candidate| candidate.contents.section_name_table_seed.push(0)),
                Box::new(|candidate| {
                    candidate.contents.descriptors.pop();
                }),
                Box::new(|candidate| candidate.contents.descriptors.swap(0, 1)),
                Box::new(|candidate| {
                    candidate.contents.descriptors[1].kind = ElfDynamicSectionKind::Interpreter
                }),
                Box::new(|candidate| candidate.contents.descriptors[0].name_offset = u32::MAX),
                Box::new(|candidate| candidate.contents.descriptors[0].section_type = SHT_STRTAB),
                Box::new(|candidate| candidate.contents.descriptors[0].flags = 0),
                Box::new(|candidate| candidate.contents.descriptors[0].payload_size += 1),
                Box::new(|candidate| candidate.contents.descriptors[0].alignment = 3),
                Box::new(|candidate| candidate.contents.descriptors[2].entry_size = 23),
                Box::new(|candidate| {
                    candidate.contents.descriptors[2].link =
                        Some(ElfDynamicSectionKind::DynamicSymbol)
                }),
                Box::new(|candidate| candidate.contents.descriptors[2].info = 0),
                Box::new(|candidate| candidate.contents.descriptors[5].info += 1),
                Box::new(|candidate| candidate.contents.descriptors[6].entry_size = 4),
                Box::new(|candidate| {
                    candidate.non_authoritative_descriptor_compatibility_fingerprint ^= 1
                }),
            ];

            for corrupt in corruptions {
                let mut candidate = candidate(TargetProfile::LinuxX64);
                corrupt(&mut candidate);
                let error = validate_candidate(candidate)
                    .expect_err("corrupt descriptor candidate must reject before sealing");
                assert_eq!(
                    error
                        .candidate
                        .payloads
                        .plan()
                        .inputs()
                        .interpreter()
                        .target(),
                    TargetProfile::LinuxX64,
                    "descriptor rejection retains exact payload custody",
                );
            }
        }

        #[test]
        fn malformed_name_and_hash_reads_reject_without_panicking() {
            assert!(name_at(&[], 0, "test name").is_err());
            assert!(name_at(b"unterminated", 0, "test name").is_err());
            assert!(name_at(SECTION_NAME_TABLE_SEED, u32::MAX, "test name").is_err());
            assert!(read_u32(&[], 4, "hash").is_err());
            assert!(read_u32(&[0; 7], 4, "hash").is_err());
            assert!(read_u32(&[0; 8], usize::MAX, "hash").is_err());
        }
    }
}

pub(crate) mod dynamic_sections {
    use target::compatibility_fingerprint::Fnv1a;

    use crate::dynamic_executable::checked::checked_u32;
    use crate::dynamic_executable::import_sections::dynamic_link::PlannedElfDynamicLinkInputs;
    use crate::imports::ElfImportLocator;
    use diagnostics::Diagnostic;

    const STB_GLOBAL: u8 = 1;
    const STT_FUNC: u8 = 2;
    const SHN_UNDEF: u16 = 0;
    const VER_NEED_CURRENT: u16 = 1;
    const FIRST_REQUIRED_VERSION: u16 = 2;
    const VERSION_INDEX_MASK: u16 = 0x7fff;
    const GNU_HASH_BUCKET_COUNT: u32 = 1;
    const GNU_HASH_SYMBOL_OFFSET: u32 = 1;
    const GNU_HASH_BLOOM_COUNT: u32 = 1;
    const GNU_HASH_BLOOM_SHIFT: u32 = 5;
    const GNU_HASH_WORD_BITS: u32 = u64::BITS;
    #[derive(Debug)]
    #[must_use = "validated ELF dynamic sections retain the exact preflight inputs"]
    pub struct ValidatedElfDynamicSectionPlan {
        inputs: PlannedElfDynamicLinkInputs,
        contents: ElfDynamicSectionContents,
    }

    impl ValidatedElfDynamicSectionPlan {
        pub const fn inputs(&self) -> &PlannedElfDynamicLinkInputs {
            &self.inputs
        }

        #[cfg(test)]
        pub(crate) fn needed_object_count(&self) -> usize {
            self.contents.needed.len()
        }

        pub(crate) fn required_version_object_count(&self) -> usize {
            let mut objects = self
                .inputs
                .imports()
                .iter()
                .filter_map(|request| match &request.locator {
                    ElfImportLocator::Versioned { object, .. } => Some(object.as_slice()),
                    ElfImportLocator::Component { .. }
                    | ElfImportLocator::StringBackedBootstrap { .. } => None,
                })
                .collect::<Vec<_>>();
            objects.sort_unstable();
            objects.dedup();
            objects.len()
        }

        #[cfg(test)]
        pub(crate) fn required_version_count(&self) -> usize {
            self.contents
                .verneed
                .iter()
                .map(|need| need.auxiliaries.len())
                .sum()
        }

        pub const fn non_authoritative_content_compatibility_fingerprint(&self) -> u64 {
            self.contents
                .non_authoritative_content_compatibility_fingerprint
        }

        pub(crate) const fn contents(&self) -> &ElfDynamicSectionContents {
            &self.contents
        }

        pub(crate) fn into_parts(self) -> (PlannedElfDynamicLinkInputs, ElfDynamicSectionContents) {
            (self.inputs, self.contents)
        }
    }

    #[derive(Debug)]
    #[must_use = "ELF dynamic-section rejection retains the exact preflight inputs"]
    pub struct ElfDynamicSectionPlanningError {
        inputs: PlannedElfDynamicLinkInputs,
        diagnostic: Diagnostic,
    }

    image::rejection_carrier!(ElfDynamicSectionPlanningError; inputs: PlannedElfDynamicLinkInputs);

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ElfDynamicSectionContents {
        pub(crate) interpreter: Vec<u8>,
        pub(crate) dynstr: Vec<u8>,
        pub(crate) dynsym: Vec<ElfDynamicSymbol>,
        pub(crate) sysv_hash: ElfSysvHash,
        pub(crate) gnu_hash: ElfGnuHash,
        pub(crate) versym: Vec<u16>,
        pub(crate) verneed: Vec<ElfVersionNeed>,
        pub(crate) needed: Vec<u32>,
        pub(crate) bindings: Vec<ElfDynamicImportBinding>,
        pub(crate) non_authoritative_content_compatibility_fingerprint: u64,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub(crate) struct ElfDynamicSymbol {
        pub(crate) name: u32,
        pub(crate) info: u8,
        pub(crate) other: u8,
        pub(crate) section_index: u16,
        pub(crate) value: u64,
        pub(crate) size: u64,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ElfSysvHash {
        pub(crate) bucket_count: u32,
        pub(crate) chain_count: u32,
        pub(crate) buckets: Vec<u32>,
        pub(crate) chains: Vec<u32>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ElfGnuHash {
        pub(crate) bucket_count: u32,
        pub(crate) symbol_offset: u32,
        pub(crate) bloom_count: u32,
        pub(crate) bloom_shift: u32,
        pub(crate) bloom: Vec<u64>,
        pub(crate) buckets: Vec<u32>,
        pub(crate) chains: Vec<u32>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ElfVersionNeed {
        pub(crate) version: u16,
        pub(crate) count: u16,
        pub(crate) file: u32,
        pub(crate) auxiliary_offset: u32,
        pub(crate) next_offset: u32,
        pub(crate) auxiliaries: Vec<ElfVersionNeedAuxiliary>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ElfVersionNeedAuxiliary {
        pub(crate) hash: u32,
        pub(crate) flags: u16,
        pub(crate) other: u16,
        pub(crate) name: u32,
        pub(crate) next_offset: u32,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ElfDynamicImportBinding {
        pub(crate) request_index: usize,
        pub(crate) compatibility_report_identity: u64,
        pub(crate) dynamic_symbol_index: u32,
        pub(crate) version_index: u16,
    }

    struct OrderedImport<'a> {
        request_index: usize,
        compatibility_report_identity: u64,
        object: &'a [u8],
        symbol: &'a [u8],
        version: &'a [u8],
    }

    struct Candidate {
        inputs: PlannedElfDynamicLinkInputs,
        contents: ElfDynamicSectionContents,
    }

    pub(crate) fn plan_elf_dynamic_sections(
        inputs: PlannedElfDynamicLinkInputs,
    ) -> Result<ValidatedElfDynamicSectionPlan, Box<ElfDynamicSectionPlanningError>> {
        checked::plan_stage!(
            plan: inputs,
            derive: derive_contents,
            contents: contents,
            error: ElfDynamicSectionPlanningError,
        )
    }

    fn derive_contents(
        inputs: &PlannedElfDynamicLinkInputs,
    ) -> Result<ElfDynamicSectionContents, Diagnostic> {
        let ordered = ordered_imports(inputs)?;
        if ordered.is_empty() {
            return Err(Diagnostic::error(
                "ELF dynamic-section planning requires at least one referenced import",
            ));
        }

        let mut interpreter = inputs.interpreter().interpreter_path().to_vec();
        interpreter.push(0);

        let mut contents = derive_import_contents(&ordered, interpreter)?;
        contents.non_authoritative_content_compatibility_fingerprint =
            non_authoritative_content_compatibility_fingerprint(inputs, &contents);
        Ok(contents)
    }

    pub(crate) fn derive_shared_import_contents(
        imports: &[crate::imports::ElfImportRequest],
    ) -> Result<ElfDynamicSectionContents, Diagnostic> {
        let ordered = ordered_import_requests(imports)?;
        derive_import_contents(&ordered, Vec::new())
    }

    fn derive_import_contents(
        ordered: &[OrderedImport<'_>],
        interpreter: Vec<u8>,
    ) -> Result<ElfDynamicSectionContents, Diagnostic> {
        let strings = canonical_strings(ordered);
        let (dynstr, string_offsets) = build_dynstr(&strings)?;
        let version_pairs = canonical_version_pairs(ordered);
        let version_indexes = assign_version_indexes(&version_pairs)?;

        let mut dynsym = vec![ElfDynamicSymbol::default()];
        let mut versym = vec![0];
        let mut bindings = Vec::with_capacity(ordered.len());
        for import in ordered {
            let dynamic_symbol_index = checked_u32(dynsym.len(), "dynamic symbol index")?;
            let version_index = version_index(&version_indexes, import.object, import.version)?;
            dynsym.push(ElfDynamicSymbol {
                name: string_offset(&string_offsets, import.symbol)?,
                info: (STB_GLOBAL << 4) | STT_FUNC,
                other: 0,
                section_index: SHN_UNDEF,
                value: 0,
                size: 0,
            });
            versym.push(version_index);
            bindings.push(ElfDynamicImportBinding {
                request_index: import.request_index,
                compatibility_report_identity: import.compatibility_report_identity,
                dynamic_symbol_index,
                version_index,
            });
        }

        let symbol_names = ordered
            .iter()
            .map(|import| import.symbol)
            .collect::<Vec<_>>();
        let sysv_hash = build_sysv_hash(&symbol_names)?;
        let gnu_hash = build_gnu_hash(&symbol_names)?;
        let objects = canonical_objects(ordered);
        let needed = objects
            .iter()
            .map(|object| string_offset(&string_offsets, object))
            .collect::<Result<Vec<_>, _>>()?;
        let verneed =
            build_version_needs(&objects, &version_pairs, &version_indexes, &string_offsets)?;

        let contents = ElfDynamicSectionContents {
            interpreter,
            dynstr,
            dynsym,
            sysv_hash,
            gnu_hash,
            versym,
            verneed,
            needed,
            bindings,
            non_authoritative_content_compatibility_fingerprint: 0,
        };
        Ok(contents)
    }

    fn ordered_imports(
        inputs: &PlannedElfDynamicLinkInputs,
    ) -> Result<Vec<OrderedImport<'_>>, Diagnostic> {
        ordered_import_requests(inputs.imports())
    }

    fn ordered_import_requests(
        imports: &[crate::imports::ElfImportRequest],
    ) -> Result<Vec<OrderedImport<'_>>, Diagnostic> {
        let mut ordered = imports
            .iter()
            .enumerate()
            .map(|(request_index, request)| match &request.locator {
                ElfImportLocator::Versioned {
                    compatibility_report_identity,
                    object,
                    symbol,
                    version,
                    ..
                } => Ok(OrderedImport {
                    request_index,
                    compatibility_report_identity: *compatibility_report_identity,
                    object,
                    symbol,
                    version,
                }),
                ElfImportLocator::Component {
                    locator,
                    object,
                    symbol,
                } => Ok(OrderedImport {
                    request_index,
                    compatibility_report_identity: locator
                        .non_authoritative_compatibility_fingerprint(),
                    object,
                    symbol,
                    version: &[],
                }),
                ElfImportLocator::StringBackedBootstrap { .. } => Err(Diagnostic::error(
                    "string-backed ELF import reached normalized dynamic-section planning",
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        ordered.sort_by(|left, right| {
            (
                left.symbol,
                left.object,
                left.version,
                left.compatibility_report_identity,
            )
                .cmp(&(
                    right.symbol,
                    right.object,
                    right.version,
                    right.compatibility_report_identity,
                ))
        });
        Ok(ordered)
    }

    fn canonical_strings(ordered: &[OrderedImport<'_>]) -> Vec<Vec<u8>> {
        let mut strings = ordered
            .iter()
            .flat_map(|import| [import.object, import.symbol, import.version])
            .filter(|string| !string.is_empty())
            .map(<[u8]>::to_vec)
            .collect::<Vec<_>>();
        strings.sort();
        strings.dedup();
        strings
    }

    fn canonical_objects(ordered: &[OrderedImport<'_>]) -> Vec<Vec<u8>> {
        let mut objects = ordered
            .iter()
            .map(|import| import.object.to_vec())
            .collect::<Vec<_>>();
        objects.sort();
        objects.dedup();
        objects
    }

    fn canonical_version_pairs(ordered: &[OrderedImport<'_>]) -> Vec<(Vec<u8>, Vec<u8>)> {
        let mut pairs = ordered
            .iter()
            .filter(|import| !import.version.is_empty())
            .map(|import| (import.object.to_vec(), import.version.to_vec()))
            .collect::<Vec<_>>();
        pairs.sort();
        pairs.dedup();
        pairs
    }

    fn build_dynstr(strings: &[Vec<u8>]) -> Result<(Vec<u8>, Vec<(Vec<u8>, u32)>), Diagnostic> {
        let mut dynstr = vec![0];
        let mut offsets = Vec::with_capacity(strings.len());
        for string in strings {
            let offset = checked_u32(dynstr.len(), "dynamic string offset")?;
            offsets.push((string.clone(), offset));
            dynstr.extend(string);
            dynstr.push(0);
        }
        Ok((dynstr, offsets))
    }

    fn string_offset(offsets: &[(Vec<u8>, u32)], string: &[u8]) -> Result<u32, Diagnostic> {
        offsets
            .binary_search_by(|(candidate, _)| candidate.as_slice().cmp(string))
            .map(|index| offsets[index].1)
            .map_err(|_| Diagnostic::error("ELF dynamic string is absent from the canonical table"))
    }

    fn assign_version_indexes(
        pairs: &[(Vec<u8>, Vec<u8>)],
    ) -> Result<Vec<((Vec<u8>, Vec<u8>), u16)>, Diagnostic> {
        pairs
            .iter()
            .enumerate()
            .map(|(index, pair)| {
                let index = u16::try_from(index)
                    .ok()
                    .and_then(|index| index.checked_add(FIRST_REQUIRED_VERSION))
                    .filter(|index| *index <= VERSION_INDEX_MASK)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "ELF required-version count exceeds the unhidden version-index domain",
                        )
                    })?;
                Ok((pair.clone(), index))
            })
            .collect()
    }

    fn version_index(
        indexes: &[((Vec<u8>, Vec<u8>), u16)],
        object: &[u8],
        version: &[u8],
    ) -> Result<u16, Diagnostic> {
        if version.is_empty() {
            return Ok(1);
        }
        indexes
            .binary_search_by(|((candidate_object, candidate_version), _)| {
                (candidate_object.as_slice(), candidate_version.as_slice()).cmp(&(object, version))
            })
            .map(|index| indexes[index].1)
            .map_err(|_| Diagnostic::error("ELF import lacks a canonical required-version index"))
    }

    pub(crate) fn build_sysv_hash(symbol_names: &[&[u8]]) -> Result<ElfSysvHash, Diagnostic> {
        let bucket_count = checked_u32(symbol_names.len().max(1), "System V hash bucket count")?;
        let chain_count = checked_u32(symbol_names.len() + 1, "System V hash chain count")?;
        let mut buckets = vec![0; bucket_count as usize];
        let mut chains = vec![0; chain_count as usize];
        for (zero_based_index, name) in symbol_names.iter().enumerate() {
            let symbol_index = checked_u32(zero_based_index + 1, "System V hash symbol index")?;
            let bucket_index = (elf_hash(name) % bucket_count) as usize;
            if buckets[bucket_index] == 0 {
                buckets[bucket_index] = symbol_index;
                continue;
            }
            let mut chain_index = buckets[bucket_index] as usize;
            while chains[chain_index] != 0 {
                chain_index = chains[chain_index] as usize;
            }
            chains[chain_index] = symbol_index;
        }
        Ok(ElfSysvHash {
            bucket_count,
            chain_count,
            buckets,
            chains,
        })
    }

    fn build_gnu_hash(symbol_names: &[&[u8]]) -> Result<ElfGnuHash, Diagnostic> {
        let mut bloom = vec![0u64; GNU_HASH_BLOOM_COUNT as usize];
        let mut chains = Vec::with_capacity(symbol_names.len());
        for name in symbol_names {
            let hash = gnu_hash(name);
            let first_bit = hash % GNU_HASH_WORD_BITS;
            let second_bit = (hash >> GNU_HASH_BLOOM_SHIFT) % GNU_HASH_WORD_BITS;
            bloom[0] |= (1u64 << first_bit) | (1u64 << second_bit);
            chains.push(hash & !1);
        }
        if let Some(last) = chains.last_mut() {
            *last |= 1;
        }
        Ok(ElfGnuHash {
            bucket_count: GNU_HASH_BUCKET_COUNT,
            symbol_offset: GNU_HASH_SYMBOL_OFFSET,
            bloom_count: GNU_HASH_BLOOM_COUNT,
            bloom_shift: GNU_HASH_BLOOM_SHIFT,
            bloom,
            buckets: vec![if symbol_names.is_empty() {
                0
            } else {
                GNU_HASH_SYMBOL_OFFSET
            }],
            chains,
        })
    }

    fn build_version_needs(
        objects: &[Vec<u8>],
        pairs: &[(Vec<u8>, Vec<u8>)],
        indexes: &[((Vec<u8>, Vec<u8>), u16)],
        strings: &[(Vec<u8>, u32)],
    ) -> Result<Vec<ElfVersionNeed>, Diagnostic> {
        let mut needs = Vec::with_capacity(objects.len());
        let objects = objects
            .iter()
            .filter(|object| pairs.iter().any(|(candidate, _)| candidate == *object))
            .collect::<Vec<_>>();
        for (object_index, object) in objects.iter().copied().enumerate() {
            let versions = pairs
                .iter()
                .filter(|(candidate, _)| candidate == object)
                .map(|(_, version)| version)
                .collect::<Vec<_>>();
            let count = u16::try_from(versions.len()).map_err(|_| {
                Diagnostic::error("ELF object version requirement count exceeds Elf64_Half")
            })?;
            let mut auxiliaries = Vec::with_capacity(versions.len());
            for (version_index_in_object, version) in versions.iter().enumerate() {
                auxiliaries.push(ElfVersionNeedAuxiliary {
                    hash: elf_hash(version),
                    flags: 0,
                    other: version_index(indexes, object, version)?,
                    name: string_offset(strings, version)?,
                    next_offset: if version_index_in_object + 1 == versions.len() {
                        0
                    } else {
                        16
                    },
                });
            }
            let group_size = 16usize
                .checked_add(16usize.checked_mul(auxiliaries.len()).ok_or_else(|| {
                    Diagnostic::error("ELF version-need auxiliary byte count overflow")
                })?)
                .ok_or_else(|| Diagnostic::error("ELF version-need group byte count overflow"))?;
            needs.push(ElfVersionNeed {
                version: VER_NEED_CURRENT,
                count,
                file: string_offset(strings, object)?,
                auxiliary_offset: 16,
                next_offset: if object_index + 1 == objects.len() {
                    0
                } else {
                    checked_u32(group_size, "ELF next version-need offset")?
                },
                auxiliaries,
            });
        }
        Ok(needs)
    }

    checked::seal_stage! {
        field: inputs,
        contents: contents,
        validated: ValidatedElfDynamicSectionPlan,
        validate: validate_contents,
    }

    fn validate_contents(
        inputs: &PlannedElfDynamicLinkInputs,
        contents: &ElfDynamicSectionContents,
    ) -> Result<(), Diagnostic> {
        let ordered = ordered_imports(inputs)?;
        let mut expected_interpreter = inputs.interpreter().interpreter_path().to_vec();
        expected_interpreter.push(0);
        require_equal(
            contents.interpreter == expected_interpreter,
            "PT_INTERP contents do not exactly equal the selected path plus one NUL terminator",
        )?;

        let strings = canonical_strings(&ordered);
        let (expected_dynstr, offsets) = build_dynstr(&strings)?;
        require_equal(
            contents.dynstr == expected_dynstr
                && contents.dynstr.first() == Some(&0)
                && contents.dynstr.last() == Some(&0),
            "ELF dynamic string table is not canonical or NUL framed",
        )?;

        require_equal(
            contents.dynsym.len() == ordered.len() + 1
                && contents.dynsym.first() == Some(&ElfDynamicSymbol::default()),
            "ELF dynamic symbol table lacks its exact reserved undefined row",
        )?;
        let pairs = canonical_version_pairs(&ordered);
        let version_indexes = assign_version_indexes(&pairs)?;
        require_equal(
            contents.versym.len() == contents.dynsym.len()
                && contents.versym.first() == Some(&0)
                && contents.bindings.len() == ordered.len(),
            "ELF symbol-version or import-index table does not parallel the dynamic symbol table",
        )?;
        for (index, import) in ordered.iter().enumerate() {
            let dynamic_symbol_index = checked_u32(index + 1, "validated dynamic symbol index")?;
            let expected_version = version_index(&version_indexes, import.object, import.version)?;
            let expected_symbol = ElfDynamicSymbol {
                name: string_offset(&offsets, import.symbol)?,
                info: (STB_GLOBAL << 4) | STT_FUNC,
                other: 0,
                section_index: SHN_UNDEF,
                value: 0,
                size: 0,
            };
            let expected_binding = ElfDynamicImportBinding {
                request_index: import.request_index,
                compatibility_report_identity: import.compatibility_report_identity,
                dynamic_symbol_index,
                version_index: expected_version,
            };
            require_equal(
                contents.dynsym.get(index + 1) == Some(&expected_symbol)
                    && contents.versym.get(index + 1) == Some(&expected_version)
                    && contents.bindings.get(index) == Some(&expected_binding),
                "ELF dynamic symbol, version, and private import indexes drifted",
            )?;
        }

        let names = ordered
            .iter()
            .map(|import| import.symbol)
            .collect::<Vec<_>>();
        require_equal(
            contents.sysv_hash == build_sysv_hash(&names)?,
            "System V symbol hash buckets or chains are not canonical",
        )?;
        validate_hash_reachability(&contents.sysv_hash, &names)?;
        require_equal(
            contents.gnu_hash == build_gnu_hash(&names)?,
            "GNU symbol hash header, bloom, buckets, or chains are not canonical",
        )?;
        validate_gnu_hash_reachability(&contents.gnu_hash, &names)?;

        let objects = canonical_objects(&ordered);
        let expected_needed = objects
            .iter()
            .map(|object| string_offset(&offsets, object))
            .collect::<Result<Vec<_>, _>>()?;
        require_equal(
            contents.needed == expected_needed,
            "ELF DT_NEEDED string-index roster is not canonical",
        )?;
        require_equal(
            contents.verneed == build_version_needs(&objects, &pairs, &version_indexes, &offsets)?,
            "ELF GNU version requirements do not match the exact object/version rows",
        )?;
        require_equal(
            contents.non_authoritative_content_compatibility_fingerprint
                == non_authoritative_content_compatibility_fingerprint(inputs, contents),
            "ELF dynamic-section content compatibility fingerprint does not replay",
        )
    }

    fn validate_hash_reachability(hash: &ElfSysvHash, names: &[&[u8]]) -> Result<(), Diagnostic> {
        require_equal(
            hash.bucket_count > 0
                && hash.bucket_count as usize == hash.buckets.len()
                && hash.chain_count as usize == hash.chains.len()
                && hash.chain_count as usize == names.len() + 1,
            "System V hash counts do not match their tables or dynamic symbols",
        )?;
        for (name_index, name) in names.iter().enumerate() {
            let wanted = checked_u32(name_index + 1, "validated hash symbol index")?;
            let mut current = hash.buckets[(elf_hash(name) % hash.bucket_count) as usize];
            let mut steps = 0usize;
            while current != 0 && current != wanted {
                require_equal(
                    (current as usize) < hash.chains.len(),
                    "System V hash chain contains an out-of-range symbol index",
                )?;
                current = hash.chains[current as usize];
                steps += 1;
                require_equal(steps <= names.len(), "System V hash chain contains a cycle")?;
            }
            require_equal(
                current == wanted,
                "System V hash lookup cannot reach one exact dynamic symbol",
            )?;
        }
        Ok(())
    }

    fn validate_gnu_hash_reachability(
        hash: &ElfGnuHash,
        names: &[&[u8]],
    ) -> Result<(), Diagnostic> {
        require_equal(
            hash.bucket_count == GNU_HASH_BUCKET_COUNT
                && hash.symbol_offset == GNU_HASH_SYMBOL_OFFSET
                && hash.bloom_count == GNU_HASH_BLOOM_COUNT
                && hash.bloom_shift == GNU_HASH_BLOOM_SHIFT
                && hash.bloom.len() == hash.bloom_count as usize
                && hash.buckets.len() == hash.bucket_count as usize
                && hash.chains.len() == names.len()
                && hash.buckets.first()
                    == Some(&if names.is_empty() {
                        0
                    } else {
                        GNU_HASH_SYMBOL_OFFSET
                    }),
            "GNU hash counts or canonical one-bucket symbol domain drifted",
        )?;
        let bloom_word = hash.bloom[0];
        for (index, name) in names.iter().enumerate() {
            let symbol_hash = gnu_hash(name);
            let first_bit = symbol_hash % GNU_HASH_WORD_BITS;
            let second_bit = (symbol_hash >> hash.bloom_shift) % GNU_HASH_WORD_BITS;
            require_equal(
                bloom_word & (1u64 << first_bit) != 0 && bloom_word & (1u64 << second_bit) != 0,
                "GNU hash bloom filter cannot admit one exact dynamic symbol",
            )?;
            let is_last = index + 1 == names.len();
            let expected_chain = (symbol_hash & !1) | u32::from(is_last);
            require_equal(
                hash.chains.get(index) == Some(&expected_chain),
                "GNU hash chain does not preserve an exact symbol hash or terminator",
            )?;
        }
        Ok(())
    }

    use super::super::checked::require as require_equal;
    use super::super::checked::{self, CandidateValidationError};

    fn elf_hash(bytes: &[u8]) -> u32 {
        let mut hash = 0u32;
        for byte in bytes {
            hash = hash.wrapping_shl(4).wrapping_add(u32::from(*byte));
            let high = hash & 0xf000_0000;
            if high != 0 {
                hash ^= high >> 24;
            }
            hash &= !high;
        }
        hash
    }

    fn gnu_hash(bytes: &[u8]) -> u32 {
        bytes.iter().fold(5381u32, |hash, byte| {
            hash.wrapping_mul(33).wrapping_add(u32::from(*byte))
        })
    }

    fn non_authoritative_content_compatibility_fingerprint(
        inputs: &PlannedElfDynamicLinkInputs,
        contents: &ElfDynamicSectionContents,
    ) -> u64 {
        let mut hash = Fnv1a::new();
        hash.bytes(b"omega.elf-dynamic-section-plan.v2");
        hash.bytes(inputs.interpreter().target().target_name().as_bytes());
        hash.bytes(
            &inputs
                .interpreter()
                .non_authoritative_compatibility_fingerprint()
                .to_le_bytes(),
        );
        hash.bytes(&contents.interpreter);
        hash.bytes(&contents.dynstr);
        for symbol in &contents.dynsym {
            hash.bytes(&symbol.name.to_le_bytes());
            hash.byte(symbol.info);
            hash.byte(symbol.other);
            hash.bytes(&symbol.section_index.to_le_bytes());
            hash.bytes(&symbol.value.to_le_bytes());
            hash.bytes(&symbol.size.to_le_bytes());
        }
        hash.bytes(&contents.sysv_hash.bucket_count.to_le_bytes());
        hash.bytes(&contents.sysv_hash.chain_count.to_le_bytes());
        for value in contents
            .sysv_hash
            .buckets
            .iter()
            .chain(&contents.sysv_hash.chains)
        {
            hash.bytes(&value.to_le_bytes());
        }
        hash.bytes(&contents.gnu_hash.bucket_count.to_le_bytes());
        hash.bytes(&contents.gnu_hash.symbol_offset.to_le_bytes());
        hash.bytes(&contents.gnu_hash.bloom_count.to_le_bytes());
        hash.bytes(&contents.gnu_hash.bloom_shift.to_le_bytes());
        for value in &contents.gnu_hash.bloom {
            hash.bytes(&value.to_le_bytes());
        }
        for value in contents
            .gnu_hash
            .buckets
            .iter()
            .chain(&contents.gnu_hash.chains)
        {
            hash.bytes(&value.to_le_bytes());
        }
        for value in &contents.versym {
            hash.bytes(&value.to_le_bytes());
        }
        for need in &contents.verneed {
            hash.bytes(&need.version.to_le_bytes());
            hash.bytes(&need.count.to_le_bytes());
            hash.bytes(&need.file.to_le_bytes());
            hash.bytes(&need.auxiliary_offset.to_le_bytes());
            hash.bytes(&need.next_offset.to_le_bytes());
            for auxiliary in &need.auxiliaries {
                hash.bytes(&auxiliary.hash.to_le_bytes());
                hash.bytes(&auxiliary.flags.to_le_bytes());
                hash.bytes(&auxiliary.other.to_le_bytes());
                hash.bytes(&auxiliary.name.to_le_bytes());
                hash.bytes(&auxiliary.next_offset.to_le_bytes());
            }
        }
        for needed in &contents.needed {
            hash.bytes(&needed.to_le_bytes());
        }
        for binding in &contents.bindings {
            hash.bytes(&binding.compatibility_report_identity.to_le_bytes());
            hash.bytes(&binding.dynamic_symbol_index.to_le_bytes());
            hash.bytes(&binding.version_index.to_le_bytes());
        }
        hash.finish()
    }

    #[cfg(test)]
    mod tests {
        use super::{
            Candidate, ElfDynamicSectionContents, ElfDynamicSymbol, ElfSysvHash,
            PlannedElfDynamicLinkInputs, SHN_UNDEF, STB_GLOBAL, STT_FUNC, VER_NEED_CURRENT,
            build_gnu_hash, derive_contents, elf_hash, gnu_hash, plan_elf_dynamic_sections,
            validate_candidate, validate_contents, validate_gnu_hash_reachability,
            validate_hash_reachability,
        };
        use crate::bytes::name_at;
        use crate::test_support::{SINGLE_SITE_IMPORTS as IMPORTS, SingleSiteImportFixture};
        use target::TargetProfile;

        fn interpreter_path(target: TargetProfile) -> &'static [u8] {
            crate::test_support::import_fixture_interpreter_path(target)
        }

        fn input(
            target: TargetProfile,
            imports: &[SingleSiteImportFixture],
        ) -> PlannedElfDynamicLinkInputs {
            input_with_interpreter(target, imports, interpreter_path(target))
        }

        fn input_with_interpreter(
            target: TargetProfile,
            imports: &[SingleSiteImportFixture],
            interpreter_path: &[u8],
        ) -> PlannedElfDynamicLinkInputs {
            crate::test_support::single_site_import_inputs(
                target,
                imports,
                "__omega_dynamic_import_",
                64,
                interpreter_path,
                "valid ELF fixture locator",
                "valid ELF fixture interpreter",
            )
        }

        fn candidate(target: TargetProfile) -> Candidate {
            let inputs = input(target, &IMPORTS);
            let contents = derive_contents(&inputs).expect("derived dynamic sections");
            Candidate { inputs, contents }
        }

        fn string_at(dynstr: &[u8], offset: u32) -> &[u8] {
            name_at(dynstr, offset, "dynamic string").expect("NUL-terminated dynamic string")
        }

        #[test]
        fn both_linux_targets_plan_complete_address_free_table_relationships() {
            for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
                let plan = plan_elf_dynamic_sections(input(target, &IMPORTS))
                    .expect("validated address-free dynamic sections");
                let contents = &plan.contents;

                assert_eq!(
                    contents.interpreter,
                    [interpreter_path(target), &[0]].concat()
                );
                assert_eq!(contents.dynstr.first(), Some(&0));
                assert_eq!(contents.dynstr.last(), Some(&0));
                assert_eq!(
                    contents.dynstr,
                    b"\0OMEGA_1\xfd\0OMEGA_2\0alpha\xfe\0beta\0gamma\0libalpha\xff.so.1\0libbeta.so.2\0"
                );
                assert_eq!(contents.dynsym.len(), IMPORTS.len() + 1);
                assert_eq!(contents.dynsym[0], ElfDynamicSymbol::default());
                assert_eq!(
                    contents
                        .dynsym
                        .iter()
                        .map(|symbol| symbol.name)
                        .collect::<Vec<_>>(),
                    [0, 18, 18, 25, 30],
                );
                assert!(contents.dynsym[1..].iter().all(|symbol| {
                    symbol.info == (STB_GLOBAL << 4) | STT_FUNC
                        && symbol.other == 0
                        && symbol.section_index == SHN_UNDEF
                        && symbol.value == 0
                        && symbol.size == 0
                        && !string_at(&contents.dynstr, symbol.name).is_empty()
                }));
                assert_eq!(contents.versym.len(), contents.dynsym.len());
                assert_eq!(contents.versym[0], 0);
                assert!(contents.versym[1..].iter().all(|version| *version >= 2));
                assert_eq!(
                    contents.sysv_hash.chain_count as usize,
                    contents.dynsym.len()
                );
                assert_eq!(contents.gnu_hash.bucket_count, 1);
                assert_eq!(contents.gnu_hash.symbol_offset, 1);
                assert_eq!(contents.gnu_hash.bloom_count, 1);
                assert_eq!(contents.gnu_hash.bloom_shift, 5);
                assert_eq!(contents.gnu_hash.bloom, [0x0000_0102_0090_2200]);
                assert_eq!(contents.gnu_hash.buckets, [1]);
                assert_eq!(
                    contents.gnu_hash.chains,
                    [0xf204_f288, 0xf204_f288, 0x7c94_89a0, 0x0f7d_eae9]
                );
                assert_eq!(contents.needed.len(), 2);
                assert_eq!(contents.needed, [36, 51]);
                assert_eq!(contents.verneed.len(), 2);
                assert_eq!(plan.required_version_count(), 3);
                assert!(contents.verneed.iter().all(|need| {
                    need.version == VER_NEED_CURRENT
                        && need.count as usize == need.auxiliaries.len()
                        && string_at(&contents.dynstr, need.file).starts_with(b"lib")
                        && need.auxiliaries.iter().all(|auxiliary| {
                            auxiliary.hash == elf_hash(string_at(&contents.dynstr, auxiliary.name))
                                && auxiliary.flags == 0
                                && auxiliary.other >= 2
                        })
                }));
                validate_contents(plan.inputs(), contents).expect("independent table replay");
                assert_ne!(
                    plan.non_authoritative_content_compatibility_fingerprint(),
                    0
                );
            }
        }

        #[test]
        fn canonical_table_contents_and_identity_ignore_import_insertion_order() {
            let forward = plan_elf_dynamic_sections(input(TargetProfile::LinuxX64, &IMPORTS))
                .expect("forward plan");
            let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
            let reverse =
                plan_elf_dynamic_sections(input(TargetProfile::LinuxX64, &reverse_imports))
                    .expect("reverse plan");

            assert_eq!(forward.contents.interpreter, reverse.contents.interpreter);
            assert_eq!(forward.contents.dynstr, reverse.contents.dynstr);
            assert_eq!(forward.contents.dynsym, reverse.contents.dynsym);
            assert_eq!(forward.contents.sysv_hash, reverse.contents.sysv_hash);
            assert_eq!(forward.contents.gnu_hash, reverse.contents.gnu_hash);
            assert_eq!(forward.contents.versym, reverse.contents.versym);
            assert_eq!(forward.contents.verneed, reverse.contents.verneed);
            assert_eq!(forward.contents.needed, reverse.contents.needed);
            assert_eq!(
                forward.non_authoritative_content_compatibility_fingerprint(),
                reverse.non_authoritative_content_compatibility_fingerprint()
            );
        }

        #[test]
        fn content_identity_binds_profile_interpreter_and_exact_table_coordinates() {
            let baseline = plan_elf_dynamic_sections(input(TargetProfile::LinuxX64, &IMPORTS))
                .expect("baseline plan")
                .non_authoritative_content_compatibility_fingerprint();
            let changed_interpreter = plan_elf_dynamic_sections(input_with_interpreter(
                TargetProfile::LinuxX64,
                &IMPORTS,
                b"/another/ld-linux-x86-64.so.2",
            ))
            .expect("changed-interpreter plan")
            .non_authoritative_content_compatibility_fingerprint();
            let changed_profile =
                plan_elf_dynamic_sections(input(TargetProfile::LinuxArm64, &IMPORTS))
                    .expect("changed-profile plan")
                    .non_authoritative_content_compatibility_fingerprint();
            let mut changed_imports = IMPORTS;
            changed_imports[0] = SingleSiteImportFixture {
                object: b"libalpha\xff.so.1",
                symbol: b"alpha_changed\xfe",
                version: b"OMEGA_1\xfd",
            };
            let changed_coordinate =
                plan_elf_dynamic_sections(input(TargetProfile::LinuxX64, &changed_imports))
                    .expect("changed-coordinate plan")
                    .non_authoritative_content_compatibility_fingerprint();

            assert_ne!(baseline, changed_interpreter);
            assert_ne!(baseline, changed_profile);
            assert_ne!(baseline, changed_coordinate);
        }

        #[test]
        fn shared_objects_versions_and_strings_deduplicate_without_losing_import_rows() {
            let plan = plan_elf_dynamic_sections(input(TargetProfile::LinuxX64, &IMPORTS))
                .expect("validated plan");
            let contents = &plan.contents;
            let alpha_version_indexes = contents
                .bindings
                .iter()
                .filter_map(|binding| {
                    let symbol = &contents.dynsym[binding.dynamic_symbol_index as usize];
                    (string_at(&contents.dynstr, symbol.name) == b"alpha\xfe")
                        .then_some(binding.version_index)
                })
                .collect::<Vec<_>>();

            assert_eq!(contents.bindings.len(), IMPORTS.len());
            assert_eq!(alpha_version_indexes.len(), 2);
            assert_ne!(alpha_version_indexes[0], alpha_version_indexes[1]);
            assert_eq!(contents.verneed[0].auxiliaries.len(), 2);
            assert_eq!(contents.verneed[1].auxiliaries.len(), 1);
            assert_eq!(contents.needed.len(), 2);
        }

        #[test]
        fn independent_validation_rejects_every_cross_table_corruption() {
            let mut corruptions: Vec<Box<dyn Fn(&mut ElfDynamicSectionContents)>> = vec![
                Box::new(|contents| {
                    contents.interpreter.pop();
                }),
                Box::new(|contents| contents.dynstr[0] = 1),
                Box::new(|contents| {
                    contents.dynstr.pop();
                }),
                Box::new(|contents| contents.dynstr[1] ^= 1),
                Box::new(|contents| contents.dynsym[1].name = 0),
                Box::new(|contents| contents.sysv_hash.buckets[0] = u32::MAX),
                Box::new(|contents| contents.sysv_hash.chains[1] = u32::MAX),
                Box::new(|contents| contents.gnu_hash.bucket_count = 0),
                Box::new(|contents| contents.gnu_hash.symbol_offset = 0),
                Box::new(|contents| contents.gnu_hash.bloom_count = 0),
                Box::new(|contents| contents.gnu_hash.bloom_shift += 1),
                Box::new(|contents| contents.gnu_hash.bloom[0] ^= 1u64 << 9),
                Box::new(|contents| contents.gnu_hash.buckets[0] = u32::MAX),
                Box::new(|contents| contents.gnu_hash.chains[0] ^= 2),
                Box::new(|contents| contents.gnu_hash.chains[3] &= !1),
                Box::new(|contents| contents.versym[1] = 1),
                Box::new(|contents| {
                    contents.verneed.pop();
                }),
                Box::new(|contents| {
                    contents.verneed.push(contents.verneed[0].clone());
                }),
                Box::new(|contents| contents.verneed[0].count += 1),
                Box::new(|contents| contents.verneed[0].auxiliary_offset = 0),
                Box::new(|contents| contents.verneed[0].next_offset = 0),
                Box::new(|contents| contents.verneed[0].auxiliaries[0].hash ^= 1),
                Box::new(|contents| contents.verneed[0].auxiliaries[0].next_offset = 0),
                Box::new(|contents| contents.needed[0] = 0),
                Box::new(|contents| contents.bindings[0].dynamic_symbol_index += 1),
                Box::new(|contents| {
                    contents.non_authoritative_content_compatibility_fingerprint ^= 1
                }),
            ];

            for corrupt in corruptions.drain(..) {
                let mut candidate = candidate(TargetProfile::LinuxX64);
                corrupt(&mut candidate.contents);
                let error = validate_candidate(candidate)
                    .expect_err("corrupt candidate must reject before sealing");
                assert_eq!(
                    error.candidate.inputs.interpreter().target(),
                    TargetProfile::LinuxX64,
                    "validation rejection retains exact input custody",
                );
            }
        }

        #[test]
        fn pure_component_import_tables_have_no_lookup_candidates() {
            let contents =
                super::derive_shared_import_contents(&[]).expect("pure component tables");
            assert_eq!(contents.dynstr, [0]);
            assert_eq!(contents.dynsym, [ElfDynamicSymbol::default()]);
            assert!(contents.needed.is_empty());
            assert!(contents.bindings.is_empty());
            assert_eq!(contents.gnu_hash.bloom, [0]);
            assert_eq!(contents.gnu_hash.buckets, [0]);
            assert!(contents.gnu_hash.chains.is_empty());
            validate_gnu_hash_reachability(&contents.gnu_hash, &[]).unwrap();
            let mut invalid = contents.gnu_hash;
            invalid.buckets[0] = 1;
            assert!(validate_gnu_hash_reachability(&invalid, &[]).is_err());
        }

        #[test]
        fn gnu_hash_matches_known_vectors_and_exact_canonical_chain() {
            assert_eq!(gnu_hash(b""), 0x0000_1505);
            assert_eq!(gnu_hash(b"exit"), 0x7c96_7e3f);
            assert_eq!(gnu_hash(b"printf"), 0x156b_2bb8);
            assert_eq!(gnu_hash(b"memcpy"), 0x0d82_7590);

            let names = [
                b"alpha\xfe".as_slice(),
                b"alpha\xfe".as_slice(),
                b"beta".as_slice(),
                b"gamma".as_slice(),
            ];
            let hash = build_gnu_hash(&names).expect("canonical GNU hash");
            assert_eq!(hash.bloom, [0x0000_0102_0090_2200]);
            assert_eq!(hash.buckets, [1]);
            assert_eq!(
                hash.chains,
                [0xf204_f288, 0xf204_f288, 0x7c94_89a0, 0x0f7d_eae9]
            );
            validate_gnu_hash_reachability(&hash, &names).expect("exact GNU hash reachability");
        }

        #[test]
        fn malformed_hash_counts_reject_without_panicking() {
            let mut zero_buckets = candidate(TargetProfile::LinuxX64);
            zero_buckets.contents.sysv_hash.bucket_count = 0;
            zero_buckets.contents.sysv_hash.buckets.clear();
            assert!(validate_candidate(zero_buckets).is_err());

            let mut mismatched_buckets = candidate(TargetProfile::LinuxX64);
            mismatched_buckets.contents.sysv_hash.bucket_count += 1;
            assert!(validate_candidate(mismatched_buckets).is_err());

            let mut mismatched_chains = candidate(TargetProfile::LinuxX64);
            mismatched_chains.contents.sysv_hash.chain_count = 0;
            mismatched_chains.contents.sysv_hash.chains.clear();
            assert!(validate_candidate(mismatched_chains).is_err());

            let names = [b"symbol".as_slice()];
            for malformed in [
                ElfSysvHash {
                    bucket_count: 0,
                    chain_count: 2,
                    buckets: Vec::new(),
                    chains: vec![0, 0],
                },
                ElfSysvHash {
                    bucket_count: 1,
                    chain_count: 0,
                    buckets: vec![1],
                    chains: Vec::new(),
                },
                ElfSysvHash {
                    bucket_count: 1,
                    chain_count: 2,
                    buckets: vec![u32::MAX],
                    chains: vec![0, 0],
                },
            ] {
                assert!(validate_hash_reachability(&malformed, &names).is_err());
            }
            let cyclic = ElfSysvHash {
                bucket_count: 1,
                chain_count: 3,
                buckets: vec![1],
                chains: vec![0, 1, 0],
            };
            assert!(
                validate_hash_reachability(&cyclic, &[b"first".as_slice(), b"second".as_slice()],)
                    .is_err()
            );

            let names = [b"symbol".as_slice()];
            let mut malformed_gnu = build_gnu_hash(&names).expect("valid GNU hash");
            malformed_gnu.bloom.clear();
            assert!(validate_gnu_hash_reachability(&malformed_gnu, &names).is_err());
            let mut malformed_gnu = build_gnu_hash(&names).expect("valid GNU hash");
            malformed_gnu.buckets[0] = u32::MAX;
            assert!(validate_gnu_hash_reachability(&malformed_gnu, &names).is_err());
            let mut malformed_gnu = build_gnu_hash(&names).expect("valid GNU hash");
            malformed_gnu.chains[0] &= !1;
            assert!(validate_gnu_hash_reachability(&malformed_gnu, &names).is_err());
        }
    }
}

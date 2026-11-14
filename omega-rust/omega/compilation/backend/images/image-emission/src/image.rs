/*
We emit executable container images (`emit_executable_image`) over a sealed
`ObjectArtifact` via the direct ELF/PE/Mach-O writers or the dynamic ELF lane
(`dynamic_elf`), attach entry-shim footprint effects, and replay terminal image
custody (`final_image_validation`) for retained-artifact verifiers.
*/

pub(crate) mod dynamic_elf {
    use diagnostics::Diagnostic;
    use image::{
        EmittedImageOutput, FinalImageInput, emitted_direct_executable_output,
        final_image_symbol_digest,
    };
    use image_elf::{
        ElfDynamicExecutableEmissionError, ValidatedElfDynamicExecutable,
        emit_elf_dynamic_executable,
    };
    use target::{Architecture, NormalizedElfInterpreterPlan, ObjectFormat};

    use crate::ObjectArtifact;
    use crate::image::final_image_validation::validate_terminal_image;

    #[derive(Debug)]
    pub struct RequestedDynamicElfImage {
        pub(crate) artifact: ObjectArtifact,
        pub(crate) emission: DynamicElfImageEmission,
    }

    impl RequestedDynamicElfImage {
        pub const fn artifact(&self) -> &ObjectArtifact {
            &self.artifact
        }

        pub const fn emission(&self) -> &DynamicElfImageEmission {
            &self.emission
        }

        pub const fn output(&self) -> &EmittedImageOutput {
            self.emission.output()
        }
    }

    pub fn validate_requested_dynamic_elf_image(
        artifact: &ObjectArtifact,
        image: &RequestedDynamicElfImage,
    ) -> Result<(), Diagnostic> {
        if artifact.target().object_format != ObjectFormat::Elf
            || artifact.object().layout.normalized_imports.is_empty()
        {
            return Err(Diagnostic::error(
                "dynamic ELF image custody requires exact normalized ELF imports",
            ));
        }
        if image.artifact != *artifact {
            return Err(Diagnostic::error(
                "dynamic ELF image custody does not retain the exact source object artifact",
            ));
        }
        validate_dynamic_elf_image_emission(artifact, &image.emission)
    }

    /// Exact admitted dynamic ELF bytes after production-emitter reconciliation.
    ///
    /// This is intentionally not [`crate::ExecutableImage`]. Dynamic ELF byte
    /// custody cannot substitute for direct-image custody; neither grants
    /// receiver-side installation or execution authority.
    ///
    /// ```compile_fail
    /// use image_emission::{RequestedDynamicElfImage, ExecutableImage};
    ///
    /// fn requires_direct_image(_: &ExecutableImage) {}
    /// fn cannot_substitute(image: &RequestedDynamicElfImage) {
    ///     requires_direct_image(image.emission());
    /// }
    /// ```
    #[derive(Debug)]
    #[must_use = "dynamic ELF production emission retains admitted byte custody"]
    pub struct DynamicElfImageEmission {
        admitted: ValidatedElfDynamicExecutable,
        output: EmittedImageOutput,
    }

    impl DynamicElfImageEmission {
        pub const fn admitted(&self) -> &ValidatedElfDynamicExecutable {
            &self.admitted
        }

        pub const fn output(&self) -> &EmittedImageOutput {
            &self.output
        }
    }

    #[derive(Debug)]
    #[must_use = "dynamic ELF emission rejection retains admitted byte custody"]
    pub struct DynamicElfImageEmissionError {
        diagnostic: Diagnostic,
    }

    impl DynamicElfImageEmissionError {
        pub const fn diagnostic(&self) -> &Diagnostic {
            &self.diagnostic
        }
    }

    diagnostics::diagnostic_display!(DynamicElfImageEmissionError);

    impl std::error::Error for DynamicElfImageEmissionError {}

    #[derive(Debug)]
    #[must_use = "dynamic ELF orchestration failure retains the exact failing owner"]
    pub enum DynamicElfOrchestrationError {
        HostedEntryPreparation(Diagnostic),
        Elf(Box<ElfDynamicExecutableEmissionError>),
        ProductionBridge(Box<DynamicElfImageEmissionError>),
    }

    impl DynamicElfOrchestrationError {
        pub const fn stage(&self) -> &'static str {
            match self {
                Self::HostedEntryPreparation(_) => "hosted-entry-preparation",
                Self::Elf(error) => error.stage(),
                Self::ProductionBridge(_) => "production-bridge",
            }
        }

        pub const fn diagnostic(&self) -> &Diagnostic {
            match self {
                Self::HostedEntryPreparation(diagnostic) => diagnostic,
                Self::Elf(error) => error.diagnostic(),
                Self::ProductionBridge(error) => error.diagnostic(),
            }
        }
    }

    impl std::fmt::Display for DynamicElfOrchestrationError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                formatter,
                "dynamic ELF {}: {}",
                self.stage(),
                self.diagnostic()
            )
        }
    }

    impl std::error::Error for DynamicElfOrchestrationError {}

    pub(crate) fn emit_dynamic_elf_image(
        artifact: &ObjectArtifact,
        interpreter: NormalizedElfInterpreterPlan,
    ) -> Result<DynamicElfImageEmission, Box<DynamicElfOrchestrationError>> {
        let prepared_entry =
            crate::entries::hosted_unit_entry::prepare(artifact).map_err(|diagnostic| {
                Box::new(DynamicElfOrchestrationError::HostedEntryPreparation(
                    diagnostic,
                ))
            })?;
        let prepared =
            crate::entries::hosted_unit_entry::prepared_parts(artifact, prepared_entry.as_ref());
        let mut image = image::build_final_image(FinalImageInput {
            target: artifact.target(),
            object: prepared.object,
            relocations: prepared.relocations,
            text_bytes: prepared.text_bytes,
            data_bytes: artifact.data_bytes(),
        });
        crate::image::entry_effects::attach(artifact, &mut image, prepared.entry_shim.as_ref())
            .map_err(|diagnostic| {
                Box::new(DynamicElfOrchestrationError::HostedEntryPreparation(
                    diagnostic,
                ))
            })?;
        let admitted = emit_elf_dynamic_executable(image, interpreter)
            .map_err(|error| Box::new(DynamicElfOrchestrationError::Elf(error)))?;
        let output = match validated_output(artifact, &prepared, &admitted) {
            Ok(output) => output,
            Err(diagnostic) => {
                return Err(Box::new(DynamicElfOrchestrationError::ProductionBridge(
                    Box::new(DynamicElfImageEmissionError { diagnostic }),
                )));
            }
        };
        Ok(DynamicElfImageEmission { admitted, output })
    }

    fn validated_output(
        artifact: &ObjectArtifact,
        prepared: &crate::entries::hosted_unit_entry::PreparedParts<'_>,
        admitted: &ValidatedElfDynamicExecutable,
    ) -> Result<EmittedImageOutput, Diagnostic> {
        let target = artifact.target();
        if target != admitted.image().target
            || target.object_format != ObjectFormat::Elf
            || !matches!(
                target.architecture,
                Architecture::Aarch64 | Architecture::X86_64
            )
        {
            return Err(Diagnostic::error(
                "admitted dynamic ELF target does not match the exact Linux object artifact",
            ));
        }
        let mut output = emitted_direct_executable_output(admitted.output().clone());
        let validation = validate_terminal_image(
            artifact,
            prepared.object,
            prepared.relocations,
            prepared.text_bytes,
            prepared.entry_shim.clone(),
            &output,
        )?;
        output.compiler_text_validation = Some(validation);
        Ok(output)
    }

    pub(crate) fn validate_dynamic_elf_image_emission(
        artifact: &ObjectArtifact,
        emission: &DynamicElfImageEmission,
    ) -> Result<(), Diagnostic> {
        crate::object::function_fragments::replay::validate(artifact)?;
        let expected = derive_output(artifact, &emission.admitted)?;
        if emission.output != expected {
            return Err(Diagnostic::error(
                "dynamic ELF production output drifted from exact admitted-byte custody",
            ));
        }
        Ok(())
    }

    fn derive_output(
        artifact: &ObjectArtifact,
        admitted: &ValidatedElfDynamicExecutable,
    ) -> Result<EmittedImageOutput, Diagnostic> {
        let prepared_entry = crate::entries::hosted_unit_entry::prepare(artifact)?;
        let prepared =
            crate::entries::hosted_unit_entry::prepared_parts(artifact, prepared_entry.as_ref());
        let output = validated_output(artifact, &prepared, admitted)?;
        let mut replayed_image = image::build_final_image(FinalImageInput {
            target: artifact.target(),
            object: prepared.object,
            relocations: prepared.relocations,
            text_bytes: prepared.text_bytes,
            data_bytes: artifact.data_bytes(),
        });
        crate::image::entry_effects::attach(
            artifact,
            &mut replayed_image,
            prepared.entry_shim.as_ref(),
        )?;
        let symbol_digest = final_image_symbol_digest(&replayed_image);
        replayed_image.memory.text = output.final_text_bytes.clone();
        replayed_image.memory.data = output.final_data_bytes.clone();
        if replayed_image != *admitted.image()
            || symbol_digest != final_image_symbol_digest(admitted.image())
        {
            return Err(Diagnostic::error(
                "admitted dynamic ELF final image does not replay from the exact object artifact",
            ));
        }
        if let Some(shim) = prepared.entry_shim.as_ref()
            && shim.surface == crate::entries::hosted_unit_entry::EntrySurface::ProvisionedUnit
        {
            crate::entries::hosted_unit_entry::validate_unit_shim(
                shim.clone(),
                artifact,
                prepared.object,
                prepared.text_bytes,
                prepared.relocations,
                &output,
            )?;
        }
        Ok(output)
    }
}

pub(crate) mod entry_effects {
    use crate::ObjectArtifact;
    use crate::entries::hosted_unit_entry::{EntryShim, EntrySurface};
    use diagnostics::Diagnostic;
    use image::{EmittedImageOutput, FinalExecutableRegionOrigin, FinalImage};

    pub(crate) fn attach(
        artifact: &ObjectArtifact,
        image: &mut FinalImage,
        shim: Option<&EntryShim>,
    ) -> Result<(), Diagnostic> {
        for private in artifact.private_functions() {
            let (handle, symbol) =
                object_file::object_function_symbol(artifact.object(), private.identity)
                    .ok_or_else(|| {
                        Diagnostic::error("entry state evidence lost its exact private identity")
                    })?;
            if handle != private.function.symbol
                || symbol.offset != private.function.text_offset
                || symbol.size != private.function.byte_count
            {
                return Err(Diagnostic::error(
                    "entry state evidence changed its exact private span",
                ));
            }
            let offset = symbol.offset;
            let size = symbol.size;
            let region = image
                .executable_regions
                .iter_mut()
                .find(|region| {
                    region.origin == FinalExecutableRegionOrigin::CompilerFunction
                        && region.section_offset == offset
                        && region.byte_count == size
                })
                .ok_or_else(|| {
                    Diagnostic::error("entry state evidence lost its exact private region")
                })?;
            region.entry_effects = Some(private.entry_effects.clone());
            if let Some(bridge) = &private.entry_bridge {
                region.footprint = Some(bridge.generated().effects.footprint().clone());
            }
        }
        if let Some(shim) = shim
            && shim.surface == EntrySurface::Physical
        {
            let (_, offset) = shim.symbol_offset();
            let (generated, effects) = crate::entries::physical_entry::generated_state_effects(
                artifact,
            )
            .ok_or_else(|| Diagnostic::error("physical entry has no generated state evidence"))?;
            let region = image
                .executable_regions
                .iter_mut()
                .find(|region| {
                    region.origin == FinalExecutableRegionOrigin::CompilerFunction
                        && region.section_offset == offset
                })
                .ok_or_else(|| {
                    Diagnostic::error("physical entry has no exact state-evidence region")
                })?;
            region.footprint = Some(generated.effects.footprint().clone());
            region.entry_effects = Some(effects.clone());
        }
        Ok(())
    }

    pub(crate) fn validate(
        artifact: &ObjectArtifact,
        output: &EmittedImageOutput,
        shim: Option<&EntryShim>,
    ) -> Result<(), Diagnostic> {
        for private in artifact.private_functions() {
            let region = output
                .executable_regions
                .regions
                .iter()
                .find(|region| {
                    region.origin == FinalExecutableRegionOrigin::CompilerFunction
                        && region.section_offset == private.function.text_offset
                        && region.byte_count == private.function.byte_count
                })
                .ok_or_else(|| {
                    Diagnostic::error("final private entry state has no exact placed region")
                })?;
            if region.entry_effects.as_ref() != Some(&private.entry_effects) {
                return Err(Diagnostic::error(
                    "final private entry state differs from its checked body and plan",
                ));
            }
            if let Some(bridge) = &private.entry_bridge {
                let body = private.body.entry_function();
                let binding = private
                    .symbol_bindings
                    .iter()
                    .find(|(_, binding)| binding.local == body.symbol)
                    .ok_or_else(|| {
                        Diagnostic::error("entry body call lost its retained body symbol")
                    })?
                    .1;
                let symbols = &artifact.object().layout.symbols;
                if !symbols.is_valid(binding.linked) {
                    return Err(Diagnostic::error("entry body call has a stale body symbol"));
                }
                let symbol = symbols.get(binding.linked);
                if symbol.section
                    != object_file::SymbolSection::Section(object_file::SectionKind::Text)
                    || symbol.kind != object_file::SymbolKind::Function
                    || Some(symbol.offset) != private.text_offset.checked_add(body.text_offset)
                    || symbol.size != body.byte_count
                {
                    return Err(Diagnostic::error(
                        "entry body call changed its exact retained body placement",
                    ));
                }
                validate_body_call_target(
                    bridge.request().target.architecture,
                    &output.final_text_bytes,
                    region.section_offset,
                    bridge.call_field_offset(),
                    symbol.offset,
                )?;
                let end = region
                    .section_offset
                    .checked_add(region.byte_count)
                    .ok_or_else(|| Diagnostic::error("final entry state byte span overflows"))?;
                let bytes = output
                    .final_text_bytes
                    .get(region.section_offset..end)
                    .ok_or_else(|| Diagnostic::error("final entry state byte span is stale"))?;
                let call = Some(
                    u32::try_from(bridge.call_field_offset())
                        .map_err(|_| Diagnostic::error("final entry body call exceeds u32"))?,
                );
                let replay = match artifact.target().architecture {
                    target::Architecture::X86_64 => {
                        isa_x86_64::decode_x86_64_generated_entry(bytes, call)
                    }
                    target::Architecture::Aarch64 => {
                        isa_aarch64::decode_aarch64_generated_entry(bytes, call)
                    }
                }
                .map_err(Diagnostic::error)?;
                if replay != *bridge.generated()
                    || region.footprint.as_ref() != Some(replay.effects.footprint())
                {
                    return Err(Diagnostic::error(
                        "final entry instructions or effects differ from target replay",
                    ));
                }
            }
        }
        if let Some(shim) = shim
            && shim.surface == EntrySurface::Physical
        {
            let (_, offset) = shim.symbol_offset();
            let (expected, effects) = crate::entries::physical_entry::generated_state_effects(
                artifact,
            )
            .ok_or_else(|| Diagnostic::error("physical entry lost its checked state evidence"))?;
            let region = output
                .executable_regions
                .regions
                .iter()
                .find(|region| {
                    region.origin == FinalExecutableRegionOrigin::CompilerFunction
                        && region.section_offset == offset
                })
                .ok_or_else(|| {
                    Diagnostic::error("physical entry state has no exact placed region")
                })?;
            let end = offset
                .checked_add(region.byte_count)
                .ok_or_else(|| Diagnostic::error("physical state span overflows"))?;
            let bytes = output
                .final_text_bytes
                .get(offset..end)
                .ok_or_else(|| Diagnostic::error("physical state span is stale"))?;
            let call = expected.instructions.iter().find_map(|row| {
                if let calling_conventions::GeneratedEntryControl::Call { field_byte_offset } =
                    row.control
                {
                    Some(field_byte_offset)
                } else {
                    None
                }
            });
            let replay = isa_x86_64::decode_x86_64_generated_entry(bytes, call)
                .map_err(Diagnostic::error)?;
            if replay != *expected
                || region.entry_effects.as_ref() != Some(effects)
                || region.footprint.as_ref() != Some(replay.effects.footprint())
            {
                return Err(Diagnostic::error(
                    "physical entry instruction/state evidence fails exact target replay",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn validate_body_call_target(
        architecture: target::Architecture,
        final_text: &[u8],
        entry_offset: usize,
        call_field_offset: usize,
        body_offset: usize,
    ) -> Result<(), Diagnostic> {
        let field = entry_offset
            .checked_add(call_field_offset)
            .ok_or_else(|| Diagnostic::error("entry body call field overflows"))?;
        let target = u64::try_from(body_offset)
            .map_err(|_| Diagnostic::error("entry body target exceeds u64"))?;
        let kind = match architecture {
            target::Architecture::X86_64 => object_file::RelocationKind::X86_64Relative32,
            target::Architecture::Aarch64 => object_file::RelocationKind::Aarch64Branch26,
        };
        image::validate_relative_relocation_target(kind, final_text, field, 0, target)
    }
}

pub(crate) mod final_image_validation {
    use diagnostics::Diagnostic;
    use image::{
        CompilerTextValidationEvidence, EmittedImageOutput, FinalExecutableRegionOrigin,
        validate_final_text_relocation_envelope,
    };

    use crate::ObjectArtifact;
    use crate::entries::hosted_unit_entry::validate_unit_shim;
    use crate::entries::hosted_unit_entry::{EntryShim, EntrySurface};
    use crate::object::function_fragments::implementation_tables::validate_image_data_relocations;
    use crate::object::private_object_composition::ObjectNamespace;
    use crate::object::private_object_composition::namespaces;
    use crate::object::private_object_composition::remap;

    mod implementation_tables {
        use diagnostics::Diagnostic;
        use image::EmittedImageOutput;
        use machine_code::ImageAddressFields;

        use crate::object::private_object_composition::ObjectNamespace;

        pub(super) fn validate_targets(
            namespace: &ObjectNamespace<'_>,
            output: &EmittedImageOutput,
        ) -> Result<(), Diagnostic> {
            let artifact = namespace.artifact;
            let Some(pool) = &artifact.implementation_table_pool else {
                return Ok(());
            };
            for address in &pool.addresses {
                let table = pool
                    .tables
                    .iter()
                    .find(|table| Some(table.table) == address.implementation_table())
                    .ok_or_else(|| {
                        Diagnostic::error("final implementation address has no table")
                    })?;
                let expected_target = section_address(
                    output.final_image_layout.data_address,
                    namespace.data_offset(table.data_offset)?,
                )?;
                let field_offset = |offset| {
                    usize::try_from(offset)
                        .ok()
                        .and_then(|offset| offset.checked_add(namespace.text_offset))
                        .ok_or_else(|| {
                            Diagnostic::error("implementation address field offset overflows")
                        })
                };
                let actual_target = match address.fields {
                    ImageAddressFields::X86_64Relative32 { patch_offset } => {
                        super::decode_x86_relative_target(output, field_offset(patch_offset)?)?
                    }
                    ImageAddressFields::Aarch64PageAddress {
                        page_patch_offset,
                        page_offset_patch_offset,
                    } => super::decode_aarch64_page_target(
                        output,
                        field_offset(page_patch_offset)?,
                        field_offset(page_offset_patch_offset)?,
                    )?,
                };
                if actual_target != expected_target {
                    return Err(Diagnostic::error(
                        "final implementation address does not resolve to its exact table",
                    ));
                }
            }
            for table in &pool.tables {
                let declaration = pool.catalog.table(table.table).ok_or_else(|| {
                    Diagnostic::error("final implementation table has no source declaration")
                })?;
                for (slot, machine) in declaration.rows.iter().enumerate() {
                    let expected_target = if let Some(machine) = machine {
                        let function = artifact.function_on_machine(*machine).ok_or_else(|| {
                            Diagnostic::error("final implementation row has no function")
                        })?;
                        section_address(
                            output.final_image_layout.text_address,
                            namespace.text_offset(function.text_offset)?,
                        )?
                    } else {
                        0
                    };
                    let row_offset = slot
                        .checked_mul(8)
                        .and_then(|offset| table.data_offset.checked_add(offset))
                        .and_then(|offset| offset.checked_add(namespace.data_offset))
                        .ok_or_else(|| {
                            Diagnostic::error("final implementation row offset overflows")
                        })?;
                    let row_end = row_offset.checked_add(8).ok_or_else(|| {
                        Diagnostic::error("final implementation row end overflows")
                    })?;
                    let actual_target = output
                        .final_data_bytes
                        .get(row_offset..row_end)
                        .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
                        .map(u64::from_le_bytes)
                        .ok_or_else(|| {
                            Diagnostic::error("final implementation row is truncated")
                        })?;
                    if actual_target != expected_target {
                        return Err(Diagnostic::error(
                            "final implementation row does not resolve to its exact function or trap",
                        ));
                    }
                }
            }
            Ok(())
        }

        fn section_address(section_address: u64, offset: usize) -> Result<u64, Diagnostic> {
            u64::try_from(offset)
                .ok()
                .and_then(|offset| section_address.checked_add(offset))
                .ok_or_else(|| Diagnostic::error("final implementation target address overflows"))
        }
    }

    pub(crate) mod native_code_layouts {
        use crate::object::private_object_composition::ObjectNamespace;
        use diagnostics::Diagnostic;
        use image::EmittedImageOutput;
        use machine_code::ImageAddressFields;
        use selected_instructions::SelectedImageAddressReference;

        pub(super) fn validate_targets(
            namespace: &ObjectNamespace<'_>,
            output: &EmittedImageOutput,
        ) -> Result<(), Diagnostic> {
            let Some(pool) = &namespace.artifact.native_code_layout_pool else {
                return Ok(());
            };
            let data_address =
                |offset: usize| -> Result<u64, Diagnostic> {
                    output
                        .final_image_layout
                        .data_address
                        .checked_add(u64::try_from(namespace.data_offset(offset)?).map_err(
                            |_| Diagnostic::error("native metadata data offset overflows"),
                        )?)
                        .ok_or_else(|| Diagnostic::error("native metadata data address overflows"))
                };
            let text_address =
                |offset: u64| -> Result<u64, Diagnostic> {
                    output
                        .final_image_layout
                        .text_address
                        .checked_add(u64::try_from(namespace.text_offset).map_err(|_| {
                            Diagnostic::error("native metadata text offset overflows")
                        })?)
                        .and_then(|base| base.checked_add(offset))
                        .ok_or_else(|| Diagnostic::error("native metadata text address overflows"))
                };
            let word = |offset: usize| -> Result<u64, Diagnostic> {
                let offset = namespace.data_offset(offset)?;
                let end = offset
                    .checked_add(8)
                    .ok_or_else(|| Diagnostic::error("native metadata word end overflows"))?;
                output
                    .final_data_bytes
                    .get(offset..end)
                    .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
                    .map(u64::from_le_bytes)
                    .ok_or_else(|| Diagnostic::error("native metadata word is truncated"))
            };
            for descriptor in &pool.layouts {
                if word(descriptor.data_offset)?
                    != text_address(descriptor.layout.entry_section_offset)?
                    || word(descriptor.data_offset + 8)?
                        != data_address(descriptor.data_offset + 24)?
                    || word(descriptor.data_offset + 16)?
                        != u64::try_from(descriptor.layout.regions.len()).map_err(|_| {
                            Diagnostic::error("native metadata region count overflows")
                        })?
                {
                    return Err(Diagnostic::error(
                        "final native descriptor differs from its exact selected entry or finite region sequence",
                    ));
                }
                for (ordinal, region) in descriptor.layout.regions.iter().enumerate() {
                    let offset = descriptor.data_offset + 24 + ordinal * 16;
                    if word(offset)? != text_address(region.section_offset)?
                        || word(offset + 8)? != region.length
                    {
                        return Err(Diagnostic::error(
                            "final native region differs from finalized instruction attribution",
                        ));
                    }
                }
            }
            for address in &pool.addresses {
                let SelectedImageAddressReference::NativeCodeLayout { machine } = address.reference
                else {
                    return Err(Diagnostic::error(
                        "native metadata address has another image object kind",
                    ));
                };
                let descriptor = pool
                    .layouts
                    .iter()
                    .find(|row| row.layout.machine == machine)
                    .ok_or_else(|| Diagnostic::error("final native descriptor is absent"))?;
                let field = |offset: u64| -> Result<usize, Diagnostic> {
                    namespace.text_offset(usize::try_from(offset).map_err(|_| {
                        Diagnostic::error("native metadata relocation field overflows")
                    })?)
                };
                let target = match address.fields {
                    ImageAddressFields::X86_64Relative32 { patch_offset } => {
                        super::decode_x86_relative_target(output, field(patch_offset)?)?
                    }
                    ImageAddressFields::Aarch64PageAddress {
                        page_patch_offset,
                        page_offset_patch_offset,
                    } => super::decode_aarch64_page_target(
                        output,
                        field(page_patch_offset)?,
                        field(page_offset_patch_offset)?,
                    )?,
                };
                if target != data_address(descriptor.data_offset)? {
                    return Err(Diagnostic::error(
                        "final native query address does not resolve to its exact descriptor",
                    ));
                }
            }
            Ok(())
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct LinkedNativeCodeLayout {
            pub private_function: Option<function_identity::MachineFunctionIdentity>,
            pub terminal_psi: terminal_psi::TerminalPsiIdentity,
            pub machine: semantic_vocabulary::MachineId,
            pub entry: u64,
            pub regions: Vec<LinkedNativeCodeRegion>,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct LinkedNativeCodeRegion {
            pub base: u64,
            pub length: u64,
        }

        pub fn inspect_native_code_layouts(
            artifact: &crate::ObjectArtifact,
            image: &crate::ExecutableImage,
        ) -> Result<Vec<LinkedNativeCodeLayout>, Diagnostic> {
            if artifact.psi() != image.psi() || artifact.target() != image.target() {
                return Err(Diagnostic::error(
                    "native layout inspection requires the image's exact object and target",
                ));
            }
            let output = image.output();
            let mut descriptions = Vec::new();
            for namespace in crate::object::private_object_composition::namespaces(artifact) {
                validate_targets(&namespace, output)?;
                let Some(pool) = &namespace.artifact.native_code_layout_pool else {
                    continue;
                };
                for descriptor in &pool.layouts {
                    let address = |offset: u64| {
                        u64::try_from(namespace.text_offset)
                            .ok()
                            .and_then(|placement| {
                                output
                                    .final_image_layout
                                    .text_address
                                    .checked_add(placement)
                            })
                            .and_then(|base| base.checked_add(offset))
                            .ok_or_else(|| {
                                Diagnostic::error("native layout inspection address overflows")
                            })
                    };
                    let regions = descriptor
                        .layout
                        .regions
                        .iter()
                        .map(|region| {
                            Ok(LinkedNativeCodeRegion {
                                base: address(region.section_offset)?,
                                length: region.length,
                            })
                        })
                        .collect::<Result<Vec<_>, Diagnostic>>()?;
                    descriptions.push(LinkedNativeCodeLayout {
                        private_function: namespace.private_function,
                        terminal_psi: namespace.artifact.psi(),
                        machine: descriptor.layout.machine,
                        entry: address(descriptor.layout.entry_section_offset)?,
                        regions,
                    });
                }
            }
            Ok(descriptions)
        }
    }

    pub(crate) fn validate_terminal_image(
        artifact: &ObjectArtifact,
        object: &object_file::ObjectPlan,
        relocations: &object_file::RelocationPlan,
        text_bytes: &[u8],
        entry_shim: Option<EntryShim>,
        output: &EmittedImageOutput,
    ) -> Result<CompilerTextValidationEvidence, Diagnostic> {
        if artifact.installed_entry_execution().is_some() {
            return Err(Diagnostic::error(
                "an installed private body cannot be emitted as an ordinary image root",
            ));
        }
        let expected_imports = object
            .layout
            .symbols
            .iter()
            .filter(|(_, symbol)| symbol.kind == object_file::SymbolKind::Import)
            .count();
        validate_terminal_image_with_import_count(
            artifact,
            object,
            relocations,
            text_bytes,
            entry_shim,
            output,
            expected_imports,
        )
    }

    fn validate_terminal_image_with_import_count(
        artifact: &ObjectArtifact,
        object: &object_file::ObjectPlan,
        relocations: &object_file::RelocationPlan,
        text_bytes: &[u8],
        entry_shim: Option<EntryShim>,
        output: &EmittedImageOutput,
        expected_imports: usize,
    ) -> Result<CompilerTextValidationEvidence, Diagnostic> {
        if artifact.target().object_format == target::ObjectFormat::MachO {
            image_macho::validate_macho_loader_mapping(
                &output.bytes,
                output.final_image_layout,
                &output.final_text_bytes,
                &output.final_data_bytes,
                output.bss_bytes,
                artifact.target(),
            )?;
            image_macho::validate_macho_object_fixups(
                object,
                relocations,
                text_bytes.len(),
                artifact.data_bytes().len(),
                output,
            )?;
            image_macho::validate_macho_import_binding_pairing(
                &output.final_text_bytes,
                &output.executable_regions,
                &output.data_regions,
                artifact.target(),
            )?;
        }
        if output.final_image_imports != expected_imports {
            return Err(Diagnostic::error(
                "terminal-Psi image import count drifted from its exact object plan",
            ));
        }
        if output.final_image_relocations != relocations.record_count() {
            return Err(Diagnostic::error(format!(
                "terminal-Psi image retained {} relocation(s), expected {}",
                output.final_image_relocations,
                relocations.record_count()
            )));
        }
        if output.final_image_layout.text_address != output.executable_regions.text_address
            || output.final_text_bytes.len() != output.executable_regions.text_byte_count
            || output.final_image_layout.text_address == 0
            || output.final_image_layout.data_address != output.data_regions.data_address
            || output.final_data_bytes.len() != output.data_regions.data_byte_count
        {
            return Err(Diagnostic::error(
                "terminal-Psi image section layout does not match its exact placed inventories",
            ));
        }
        image::validate_placed_executable_region_inventory(
            &output.executable_regions,
            &output.final_text_bytes,
        )?;
        image::validate_placed_data_region_inventory(
            &output.data_regions,
            &output.final_data_bytes,
        )?;
        crate::image::entry_effects::validate(artifact, output, entry_shim.as_ref())?;
        if !output.final_data_bytes.is_empty() {
            let text_end = output
                .final_image_layout
                .text_address
                .checked_add(output.final_text_bytes.len() as u64)
                .ok_or_else(|| Diagnostic::error("terminal-Psi final text placement overflows"))?;
            if output.final_image_layout.data_address < text_end
                || !output.final_image_layout.data_address.is_multiple_of(8)
            {
                return Err(Diagnostic::error(
                    "terminal-Psi initialized data has an invalid final placement",
                ));
            }
        }
        if let Some(gap) = output.executable_regions.unclassified_gaps.first() {
            return Err(Diagnostic::error(format!(
                "terminal-Psi executable inventory left {} unclassified byte(s) at .text offset {}",
                gap.byte_count, gap.section_offset
            )));
        }
        if let Some(gap) = output.data_regions.unclassified_gaps.first() {
            return Err(Diagnostic::error(format!(
                "terminal-Psi data inventory left {} unclassified byte(s) at .data offset {}",
                gap.byte_count, gap.section_offset
            )));
        }
        let compiler_regions = output
            .executable_regions
            .regions
            .iter()
            .filter(|region| region.origin == FinalExecutableRegionOrigin::CompilerFunction)
            .collect::<Vec<_>>();
        let expected_region_count = artifact.functions.len()
            + artifact
                .private_functions
                .iter()
                .map(|private| {
                    private.body.functions.len() + usize::from(private.entry_bridge.is_some())
                })
                .sum::<usize>()
            + usize::from(entry_shim.is_some());
        if compiler_regions.len() != expected_region_count {
            return Err(Diagnostic::error(format!(
                "terminal-Psi image retained {} compiler function region(s), expected {}",
                compiler_regions.len(),
                expected_region_count
            )));
        }
        for function in &artifact.functions {
            let symbol = object_file::object_symbol_name(object, function.symbol);
            let matching = compiler_regions
                .iter()
                .filter(|region| {
                    region.symbol == symbol
                        && region.section_offset == function.text_offset
                        && region.byte_count == function.byte_count
                })
                .count();
            if matching != 1 {
                return Err(Diagnostic::error(format!(
                    "terminal-Psi function {} must bind exactly one final executable region; found {matching}",
                    function.machine
                )));
            }
        }
        for private in &artifact.private_functions {
            let function = &private.function;
            let Some((symbol_handle, symbol_plan)) =
                object_file::object_function_symbol(object, private.identity)
            else {
                return Err(Diagnostic::error(
                    "compiler-private callback function has no exact object identity binding",
                ));
            };
            if symbol_handle != function.symbol
                || symbol_plan.offset != function.text_offset
                || symbol_plan.size != function.byte_count
            {
                return Err(Diagnostic::error(
                    "compiler-private callback function identity changed its exact object span",
                ));
            }
            let matching = compiler_regions
                .iter()
                .filter(|region| {
                    region.symbol == symbol_plan.name
                        && region.section_offset == function.text_offset
                        && region.byte_count == function.byte_count
                })
                .count();
            if matching != 1 {
                return Err(Diagnostic::error(format!(
                    "compiler-private callback function must bind exactly one final executable region; found {matching}"
                )));
            }
            for helper in private.body.functions.iter().filter(|helper| {
                helper.machine != private.body.entry || private.entry_bridge.is_some()
            }) {
                let symbol = remap(&private.symbol_bindings, helper.symbol)
                    .map_err(|error| Diagnostic::error(error.to_string()))?;
                let name = object_file::object_symbol_name(object, symbol);
                let offset = private
                    .text_offset
                    .checked_add(helper.text_offset)
                    .ok_or_else(|| Diagnostic::error("private helper text offset overflows"))?;
                if compiler_regions
                    .iter()
                    .filter(|region| {
                        region.symbol == name
                            && region.section_offset == offset
                            && region.byte_count == helper.byte_count
                    })
                    .count()
                    != 1
                {
                    return Err(Diagnostic::error(
                        "private helper has no exact final executable region",
                    ));
                }
            }
        }
        if let Some(shim) = entry_shim {
            match shim.surface {
                EntrySurface::ProvisionedUnit => {}
                EntrySurface::Physical => {
                    let (symbol, offset) = shim.symbol_offset();
                    crate::entries::physical_entry::validate_image(
                        artifact,
                        object,
                        text_bytes,
                        relocations,
                        symbol,
                        offset,
                        output,
                    )?;
                }
                EntrySurface::DarwinReceiver
                | EntrySurface::LinuxReceiver
                | EntrySurface::LinuxArm64Receiver
                | EntrySurface::WindowsReceiver => {
                    crate::entries::hosted_receiver::validate_image(
                        artifact,
                        object,
                        text_bytes,
                        relocations,
                        shim,
                        output,
                    )?;
                }
                EntrySurface::DarwinUnit
                | EntrySurface::DarwinX86_64Unit
                | EntrySurface::WindowsUnit
                | EntrySurface::LinuxUnit
                | EntrySurface::LinuxArm64Unit => {
                    validate_unit_shim(shim, artifact, object, text_bytes, relocations, output)?
                }
            }
        }
        let evidence = validate_final_text_relocation_envelope(
            text_bytes,
            &output.final_text_bytes,
            relocations,
        )?;
        if output.data_bytes != output.final_data_bytes.len() {
            return Err(Diagnostic::error(
                "terminal-Psi image initialized-data count does not match its exact final bytes",
            ));
        }
        let encoded_final_data =
            encoded_final_data_with_image_import_slots(artifact, object, relocations)?;
        output.validate_final_initialized_data_relocation_envelope(
            &encoded_final_data,
            relocations,
        )?;
        validate_image_data_relocations(artifact, object, relocations)
            .map_err(|error| Diagnostic::error(error.to_string()))?;
        for namespace in namespaces(artifact) {
            implementation_tables::validate_targets(&namespace, output)?;
            native_code_layouts::validate_targets(&namespace, output)?;
            validate_byte_literal_address_targets(&namespace, output)?;
        }
        validate_callback_relocation_targets(artifact, object, relocations, output)?;
        Ok(evidence)
    }

    fn validate_byte_literal_address_targets(
        namespace: &ObjectNamespace<'_>,
        output: &EmittedImageOutput,
    ) -> Result<(), Diagnostic> {
        let artifact = namespace.artifact;
        let Some(pool) = artifact.byte_literal_pool() else {
            return Ok(());
        };
        let final_pool = output
            .final_text_bytes
            .get(
                namespace.text_offset(pool.text_offset)?
                    ..namespace.text_offset(
                        pool.text_offset
                            .checked_add(pool.byte_count)
                            .ok_or_else(|| Diagnostic::error("literal pool range overflows"))?,
                    )?,
            )
            .ok_or_else(|| Diagnostic::error("final byte-literal pool is truncated"))?;
        if artifact.text_bytes().get(
            pool.text_offset
                ..pool
                    .text_offset
                    .checked_add(pool.byte_count)
                    .ok_or_else(|| Diagnostic::error("literal pool range overflows"))?,
        ) != Some(final_pool)
        {
            return Err(Diagnostic::error(
                "final byte-literal pool differs from its object octets",
            ));
        }
        let to_offset = |offset: u64| {
            usize::try_from(offset)
                .ok()
                .and_then(|offset| offset.checked_add(namespace.text_offset))
                .ok_or_else(|| Diagnostic::error("byte-literal address field offset overflows"))
        };
        for address in &pool.addresses {
            let expected_target = output
                .executable_regions
                .text_address
                .checked_add(namespace.text_offset(pool.text_offset)? as u64)
                .and_then(|pool_address| pool_address.checked_add(address.pool_offset as u64))
                .ok_or_else(|| Diagnostic::error("byte-literal target address overflows"))?;
            let actual_target = match address.fields {
                machine_code::ImageAddressFields::X86_64Relative32 { patch_offset } => {
                    decode_x86_relative_target(output, to_offset(patch_offset)?)?
                }
                machine_code::ImageAddressFields::Aarch64PageAddress {
                    page_patch_offset,
                    page_offset_patch_offset,
                } => decode_aarch64_page_target(
                    output,
                    to_offset(page_patch_offset)?,
                    to_offset(page_offset_patch_offset)?,
                )?,
            };
            if actual_target != expected_target {
                return Err(Diagnostic::error(
                    "final byte-literal address does not resolve to its pool octets",
                ));
            }
        }
        Ok(())
    }

    fn encoded_final_data_with_image_import_slots(
        artifact: &ObjectArtifact,
        object: &object_file::ObjectPlan,
        relocations: &object_file::RelocationPlan,
    ) -> Result<Vec<u8>, Diagnostic> {
        let mut encoded = artifact.data_bytes().to_vec();
        if artifact.target().object_format != target::ObjectFormat::MachO {
            return Ok(encoded);
        }

        let referenced_import_count = object
            .layout
            .symbols
            .iter()
            .filter(|(handle, symbol)| {
                symbol.kind == object_file::SymbolKind::Import
                    && relocations
                        .records()
                        .any(|(_, relocation)| relocation.symbol_handle == *handle)
            })
            .count();
        for _ in 0..referenced_import_count {
            let aligned = encoded
                .len()
                .checked_add(7)
                .map(|length| length & !7)
                .ok_or_else(|| Diagnostic::error("Mach-O import-pointer alignment overflows"))?;
            let end = aligned
                .checked_add(8)
                .ok_or_else(|| Diagnostic::error("Mach-O import-pointer data size overflows"))?;
            encoded.resize(end, 0);
        }
        Ok(encoded)
    }

    fn validate_callback_relocation_targets(
        artifact: &ObjectArtifact,
        object: &object_file::ObjectPlan,
        relocations: &object_file::RelocationPlan,
        output: &EmittedImageOutput,
    ) -> Result<(), Diagnostic> {
        for namespace in namespaces(artifact) {
            for call in &namespace.artifact.foreign_calls {
                for callback in &call.callback_addresses {
                    let function = namespace
                        .artifact
                        .function_on_machine(call.machine)
                        .ok_or_else(|| Diagnostic::error(
                            "callback registrar call has no exact semantic function in the final image",
                        ))?;
                    let operation = match call.owner {
                        target_operations::CallSiteOwner::Operation(operation) => operation,
                        target_operations::CallSiteOwner::InvokeNoNormal(_)
                        | target_operations::CallSiteOwner::Installation { .. }
                        | target_operations::CallSiteOwner::CleanupAction { .. }
                        | target_operations::CallSiteOwner::ExternalCompletion(_) => {
                            return Err(Diagnostic::error(
                                "callback address relocation is not owned by a semantic registrar operation",
                            ));
                        }
                    };
                    let (callback_symbol, callback_symbol_plan) = object_file::object_function_symbol(
                        object,
                        callback.target.callback_function,
                    )
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "callback address relocation lost its compiler-private function symbol",
                        )
                    })?;
                    let relocation_text_offset = |relative: usize| -> Result<usize, Diagnostic> {
                        if relative
                            .checked_add(4)
                            .is_none_or(|end| end > function.byte_count)
                        {
                            return Err(Diagnostic::error(
                                "callback relocation field escapes its registrar function",
                            ));
                        }
                        namespace.text_offset(
                            function.text_offset.checked_add(relative).ok_or_else(|| {
                                Diagnostic::error("callback relocation function offset overflows")
                            })?,
                        )
                    };
                    let expected_origin = object_file::RelocationOrigin::SemanticOperation {
                        function_symbol_handle: namespace
                            .symbol(function.symbol)
                            .map_err(|error| Diagnostic::error(error.to_string()))?,
                        operation_identity: operation.get(),
                    };
                    let expected = match callback.encoding {
                        machine_code::CallbackAddressEncoding::X86_64Relative32 {
                            relocation_offset,
                        } => {
                            vec![(
                                relocation_offset,
                                object_file::RelocationKind::X86_64Relative32,
                            )]
                        }
                        machine_code::CallbackAddressEncoding::Aarch64PageAddress {
                            page_relocation_offset,
                            page_offset_relocation_offset,
                        } => vec![
                            (
                                page_relocation_offset,
                                object_file::RelocationKind::Aarch64Page21,
                            ),
                            (
                                page_offset_relocation_offset,
                                object_file::RelocationKind::Aarch64PageOffset12,
                            ),
                        ],
                    };
                    let expected = expected
                        .into_iter()
                        .map(|(offset, kind)| {
                            relocation_text_offset(offset).map(|offset| (offset, kind))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let private_target_relocations = relocations
                        .records()
                        .filter(|(_, relocation)| {
                            relocation.symbol_handle == callback_symbol
                                && relocation.origin == expected_origin
                        })
                        .map(|(_, relocation)| relocation)
                        .collect::<Vec<_>>();
                    if private_target_relocations.len() != expected.len()
                        || expected.iter().any(|(offset, kind)| {
                            private_target_relocations
                                .iter()
                                .filter(|relocation| {
                                    relocation.origin == expected_origin
                                        && relocation.section == object_file::SectionKind::Text
                                        && relocation.offset == *offset
                                        && relocation.byte_width == 4
                                        && relocation.addend == 0
                                        && relocation.kind == *kind
                                })
                                .count()
                                != 1
                        })
                    {
                        return Err(Diagnostic::error(
                            "callback address relocation set does not exactly target its private function",
                        ));
                    }
                    let expected_target = output
                        .executable_regions
                        .text_address
                        .checked_add(callback_symbol_plan.offset as u64)
                        .ok_or_else(|| {
                            Diagnostic::error("callback private function address overflows")
                        })?;
                    let actual_target = match callback.encoding {
                        machine_code::CallbackAddressEncoding::X86_64Relative32 {
                            relocation_offset,
                        } => decode_x86_relative_target(
                            output,
                            relocation_text_offset(relocation_offset)?,
                        )?,
                        machine_code::CallbackAddressEncoding::Aarch64PageAddress {
                            page_relocation_offset,
                            page_offset_relocation_offset,
                        } => decode_aarch64_page_target(
                            output,
                            relocation_text_offset(page_relocation_offset)?,
                            relocation_text_offset(page_offset_relocation_offset)?,
                        )?,
                    };
                    if actual_target != expected_target {
                        return Err(Diagnostic::error(
                            "final callback address does not resolve to its exact private function",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn decode_x86_relative_target(
        output: &EmittedImageOutput,
        relocation_offset: usize,
    ) -> Result<u64, Diagnostic> {
        let displacement = output
            .final_text_bytes
            .get(relocation_offset..relocation_offset.saturating_add(4))
            .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
            .map(i32::from_le_bytes)
            .ok_or_else(|| Diagnostic::error("final callback rel32 field is truncated"))?;
        output
            .executable_regions
            .text_address
            .checked_add(relocation_offset as u64)
            .and_then(|address| address.checked_add(4))
            .and_then(|instruction_end| instruction_end.checked_add_signed(i64::from(displacement)))
            .ok_or_else(|| Diagnostic::error("final callback rel32 target overflows"))
    }

    fn decode_aarch64_page_target(
        output: &EmittedImageOutput,
        page_relocation_offset: usize,
        page_offset_relocation_offset: usize,
    ) -> Result<u64, Diagnostic> {
        let read_word = |offset: usize| {
            output
                .final_text_bytes
                .get(offset..offset.saturating_add(4))
                .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
                .map(u32::from_le_bytes)
                .ok_or_else(|| {
                    Diagnostic::error("final callback AArch64 address field is truncated")
                })
        };
        let adrp = read_word(page_relocation_offset)?;
        let add = read_word(page_offset_relocation_offset)?;
        let immediate = (((adrp >> 5) & 0x7ffff) << 2) | ((adrp >> 29) & 0b11);
        let signed_pages = i64::from((immediate << 11) as i32 >> 11);
        let instruction_page = output
            .executable_regions
            .text_address
            .checked_add(page_relocation_offset as u64)
            .map(|address| address & !0xfff)
            .ok_or_else(|| Diagnostic::error("final callback ADRP address overflows"))?;
        let page_target = instruction_page
            .checked_add_signed(
                signed_pages
                    .checked_mul(4096)
                    .ok_or_else(|| Diagnostic::error("final callback ADRP page delta overflows"))?,
            )
            .ok_or_else(|| Diagnostic::error("final callback ADRP target overflows"))?;
        let page_offset = u64::from((add >> 10) & 0xfff);
        page_target
            .checked_add(page_offset)
            .ok_or_else(|| Diagnostic::error("final callback ADD target overflows"))
    }

    #[cfg(test)]
    mod tests {
        use super::{decode_aarch64_page_target, decode_x86_relative_target};
        use image::{
            EmittedImageOutput, FinalExecutableTextDigest, FinalImageLayout, ImageOutputKind,
            PlacedDataRegionInventory, PlacedExecutableRegionInventory,
            PlacedExecutableRegionInventoryDigest,
        };

        fn emitted_output(final_text_bytes: Vec<u8>, text_address: u64) -> EmittedImageOutput {
            EmittedImageOutput {
                bytes: Vec::new(),
                final_image_layout: FinalImageLayout {
                    text_address,
                    data_address: 0,
                    bss_address: 0,
                },
                final_text_bytes,
                final_data_bytes: Vec::new(),
                final_import_data_bytes: Vec::new(),
                callback_placement_identity_report_fingerprint: 0,
                file_name: String::new(),
                format: String::new(),
                kind: ImageOutputKind::DirectExecutable,
                text_bytes: 0,
                data_bytes: 0,
                bss_bytes: 0,
                symbols: 0,
                relocations: 0,
                final_image_symbols: 0,
                final_image_imports: 0,
                final_image_relocations: 0,
                executable_regions: PlacedExecutableRegionInventory {
                    text_address,
                    text_byte_count: 0,
                    text_digest: FinalExecutableTextDigest::from_digest([0x11; 32]),
                    text_report_fingerprint: 0,
                    inventory_digest: PlacedExecutableRegionInventoryDigest::from_digest(
                        [0x22; 32],
                    ),
                    inventory_report_fingerprint: 0,
                    regions: Vec::new(),
                    unclassified_gaps: Vec::new(),
                },
                data_regions: PlacedDataRegionInventory::empty(),
                import_data_regions: PlacedDataRegionInventory::empty(),
                compiler_text_validation: None,
                compiler_function_validation: None,
                compiler_entry_region_binding: None,
                compiler_entry_footprint_binding: None,
            }
        }

        #[test]
        fn x86_relative_target_resolves_forward_displacement() {
            let mut text = vec![0u8; 16];
            text[4..4 + 4].copy_from_slice(&0x40i32.to_le_bytes());
            let output = emitted_output(text, 0x1000);

            assert_eq!(decode_x86_relative_target(&output, 4).unwrap(), 0x1048);
        }

        #[test]
        fn x86_relative_target_resolves_backward_displacement() {
            let mut text = vec![0u8; 16];
            text[4..8].copy_from_slice(&(-8i32).to_le_bytes());
            let output = emitted_output(text, 0x1000);

            assert_eq!(decode_x86_relative_target(&output, 4).unwrap(), 0x1000);
        }

        #[test]
        fn x86_relative_target_rejects_truncated_fields() {
            let output = emitted_output(vec![0u8; 6], 0x1000);

            assert!(decode_x86_relative_target(&output, 4).is_err());
            assert!(decode_x86_relative_target(&output, 16).is_err());
        }

        #[test]
        fn x86_relative_target_rejects_address_overflow() {
            let mut text = vec![0u8; 8];
            text[0..4].copy_from_slice(&i32::MAX.to_le_bytes());
            let output = emitted_output(text, u64::MAX - 3);

            assert!(decode_x86_relative_target(&output, 0).is_err());
        }

        fn adrp_word(page_delta: i64) -> u32 {
            let value = page_delta as u32 & 0x1f_ffff;
            ((value >> 2) << 5) | ((value & 0b11) << 29)
        }

        fn add_imm_word(page_offset: u32) -> u32 {
            page_offset << 10
        }

        #[test]
        fn aarch64_page_target_resolves_forward_page_and_offset() {
            let mut text = vec![0u8; 0x20];
            text[0x10..0x10 + 4].copy_from_slice(&adrp_word(3).to_le_bytes());
            text[0x14..0x14 + 4].copy_from_slice(&add_imm_word(0x234).to_le_bytes());
            let output = emitted_output(text, 0x4000);

            assert_eq!(
                decode_aarch64_page_target(&output, 0x10, 0x14).unwrap(),
                0x7234
            );
        }

        #[test]
        fn aarch64_page_target_sign_extends_negative_page_delta() {
            let mut text = vec![0u8; 0x20];
            text[0x10..0x14].copy_from_slice(&adrp_word(-2).to_le_bytes());
            text[0x14..0x14 + 4].copy_from_slice(&add_imm_word(0x10).to_le_bytes());
            let output = emitted_output(text, 0x4000);

            assert_eq!(
                decode_aarch64_page_target(&output, 0x10, 0x14).unwrap(),
                0x2010
            );
        }

        #[test]
        fn aarch64_page_target_uses_the_instruction_page_base() {
            let mut text = vec![0u8; 0x20];
            text[0x14..0x14 + 4].copy_from_slice(&adrp_word(0).to_le_bytes());
            text[0x18..0x18 + 4].copy_from_slice(&add_imm_word(0x8).to_le_bytes());
            let output = emitted_output(text, 0x4000);

            assert_eq!(
                decode_aarch64_page_target(&output, 0x14, 0x18).unwrap(),
                0x4008
            );
        }

        #[test]
        fn aarch64_page_target_rejects_truncated_fields() {
            let output = emitted_output(vec![0u8; 0x14], 0x4000);

            assert!(decode_aarch64_page_target(&output, 0x10, 0x14).is_err());
            assert!(decode_aarch64_page_target(&output, 0x10, 0x20).is_err());
        }
    }
}

pub(crate) mod image_output {
    mod application_metadata {
        use super::ExecutableImageEmissionRequest;
        use target::{ApplicationIdentifier, HostedApplicationIntent, NativeTarget, ObjectFormat};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ImageApplicationMetadataError {
            SubsystemMismatch,
            IdentifierMismatch,
            MissingGuiIdentifier,
        }

        impl std::fmt::Display for ImageApplicationMetadataError {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(match self {
                    Self::SubsystemMismatch => {
                        "image request subsystem differs from application metadata"
                    }
                    Self::IdentifierMismatch => {
                        "image request code signature identifier differs from application metadata"
                    }
                    Self::MissingGuiIdentifier => {
                        "signed macOS GUI image emission requires an application identifier"
                    }
                })
            }
        }

        impl std::error::Error for ImageApplicationMetadataError {}

        impl ExecutableImageEmissionRequest {
            pub fn settle_application_metadata(
                &mut self,
                target: NativeTarget,
                intent: Option<HostedApplicationIntent>,
                subsystem: u16,
                identifier: Option<&ApplicationIdentifier>,
            ) -> Result<(), ImageApplicationMetadataError> {
                if let Self::Direct {
                    subsystem: requested_subsystem,
                    code_signature_identifier,
                } = self
                {
                    if *requested_subsystem != subsystem {
                        return Err(ImageApplicationMetadataError::SubsystemMismatch);
                    }
                    if let (Some(authored), Some(requested)) =
                        (identifier, code_signature_identifier.as_deref())
                        && requested != authored.as_str()
                    {
                        return Err(ImageApplicationMetadataError::IdentifierMismatch);
                    }
                    if let Some(authored) = identifier
                        && code_signature_identifier.is_none()
                    {
                        *code_signature_identifier = Some(authored.as_str().to_owned());
                    }
                }
                if target.object_format == ObjectFormat::MachO
                    && intent == Some(HostedApplicationIntent::Gui)
                    && self.code_signature_identifier().is_none()
                {
                    return Err(ImageApplicationMetadataError::MissingGuiIdentifier);
                }
                Ok(())
            }
        }
    }

    pub use application_metadata::ImageApplicationMetadataError;

    use diagnostics::Diagnostic;
    use image::{EmittedImageOutput, FinalImageInput, emitted_direct_executable_output};
    use object_file::ObjectContainerOutput;
    use target::{Architecture, NativeTarget, NormalizedElfInterpreterPlan, ObjectFormat};
    use terminal_psi::TerminalPsiIdentity;

    use crate::image::dynamic_elf::{
        DynamicElfOrchestrationError, RequestedDynamicElfImage, emit_dynamic_elf_image,
    };
    use crate::image::final_image_validation::validate_terminal_image;
    use crate::{
        ObjectArtifact, ObjectBoundarySettlement, ObjectCodeAttribution,
        ObjectCompilerPrivateFunction, ObjectFunction, ObjectPortEffect,
    };

    #[derive(Debug, Clone)]
    pub enum ExecutableImageEmissionRequest {
        Direct {
            subsystem: u16,
            code_signature_identifier: Option<String>,
        },
        DynamicElf {
            interpreter: NormalizedElfInterpreterPlan,
        },
    }

    impl ExecutableImageEmissionRequest {
        pub const fn direct(subsystem: u16) -> Self {
            Self::Direct {
                subsystem,
                code_signature_identifier: None,
            }
        }

        pub fn code_signature_identifier(&self) -> Option<&str> {
            match self {
                Self::Direct {
                    code_signature_identifier,
                    ..
                } => code_signature_identifier.as_deref(),
                Self::DynamicElf { .. } => None,
            }
        }

        pub const fn dynamic_elf(interpreter: NormalizedElfInterpreterPlan) -> Self {
            Self::DynamicElf { interpreter }
        }

        pub fn settle_elf_loader(
            self,
            artifact: &ObjectArtifact,
            elf_interpreter: Option<&NormalizedElfInterpreterPlan>,
        ) -> Result<Self, Box<RequestedExecutableImageError>> {
            if !requires_dynamic_elf(artifact) {
                return Ok(self);
            }
            let Self::Direct { subsystem, .. } = self else {
                return Ok(self);
            };
            match elf_interpreter {
                Some(interpreter) if interpreter.target().native_target() == artifact.target() => {
                    Ok(Self::dynamic_elf(interpreter.clone()))
                }
                Some(interpreter) => Err(Box::new(
                    RequestedExecutableImageError::UnexpectedDynamicElfInterpreter {
                        interpreter: interpreter.clone(),
                        diagnostic: Diagnostic::error(format!(
                            "ELF interpreter selected for target `{}` cannot load an object emitted for {:?}",
                            interpreter.target().target_name(),
                            artifact.target(),
                        )),
                    },
                )),
                None => Err(Box::new(
                    RequestedExecutableImageError::MissingDynamicElfInterpreter {
                        target: artifact.target(),
                        subsystem,
                        diagnostic: Diagnostic::error(
                            "missing output configuration: dynamically linked ELF output requires an interpreter pathname; select `Build.elf_interpreter` for the target, whose environment declares no default",
                        ),
                    },
                )),
            }
        }
    }

    fn requires_dynamic_elf(artifact: &ObjectArtifact) -> bool {
        artifact.target().object_format == ObjectFormat::Elf
            && !artifact.object().layout.normalized_imports.is_empty()
    }

    #[derive(Debug)]
    #[must_use = "requested image emission retains its exact authority boundary"]
    pub enum RequestedExecutableImage {
        Direct(ExecutableImage),
        DynamicElf(RequestedDynamicElfImage),
    }

    impl RequestedExecutableImage {
        pub const fn output(&self) -> &EmittedImageOutput {
            match self {
                Self::Direct(image) => image.output(),
                Self::DynamicElf(image) => image.output(),
            }
        }
    }

    #[derive(Debug)]
    #[must_use = "requested image-emission rejection may retain loader custody"]
    pub enum RequestedExecutableImageError {
        MissingDynamicElfInterpreter {
            target: target::NativeTarget,
            subsystem: u16,
            diagnostic: Diagnostic,
        },
        UnexpectedDynamicElfInterpreter {
            interpreter: NormalizedElfInterpreterPlan,
            diagnostic: Diagnostic,
        },
        Direct(Diagnostic),
        DynamicElf(Box<DynamicElfOrchestrationError>),
    }

    impl RequestedExecutableImageError {
        pub const fn diagnostic(&self) -> &Diagnostic {
            match self {
                Self::MissingDynamicElfInterpreter { diagnostic, .. }
                | Self::UnexpectedDynamicElfInterpreter { diagnostic, .. }
                | Self::Direct(diagnostic) => diagnostic,
                Self::DynamicElf(error) => error.diagnostic(),
            }
        }
    }

    impl std::fmt::Display for RequestedExecutableImageError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.diagnostic().fmt(formatter)
        }
    }

    impl std::error::Error for RequestedExecutableImageError {}

    pub fn emit_executable_image(
        artifact: &ObjectArtifact,
        request: ExecutableImageEmissionRequest,
    ) -> Result<RequestedExecutableImage, Box<RequestedExecutableImageError>> {
        crate::object::private_object_composition::require_definitions(&artifact.object).map_err(
            |error| {
                Box::new(RequestedExecutableImageError::Direct(Diagnostic::error(
                    error.to_string(),
                )))
            },
        )?;
        match (requires_dynamic_elf(artifact), request) {
            (true, ExecutableImageEmissionRequest::DynamicElf { interpreter }) => {
                emit_dynamic_elf_image(artifact, interpreter)
                    .map(|emission| {
                        RequestedExecutableImage::DynamicElf(RequestedDynamicElfImage {
                            artifact: artifact.clone(),
                            emission,
                        })
                    })
                    .map_err(|error| Box::new(RequestedExecutableImageError::DynamicElf(error)))
            }
            (true, ExecutableImageEmissionRequest::Direct { subsystem, .. }) => Err(Box::new(
                RequestedExecutableImageError::MissingDynamicElfInterpreter {
                    target: artifact.target(),
                    subsystem,
                    diagnostic: Diagnostic::error(
                        "import-bearing ELF image emission requires an exact normalized interpreter input",
                    ),
                },
            )),
            (false, ExecutableImageEmissionRequest::DynamicElf { interpreter }) => Err(Box::new(
                RequestedExecutableImageError::UnexpectedDynamicElfInterpreter {
                    interpreter,
                    diagnostic: Diagnostic::error(
                        "a normalized ELF interpreter cannot select the dynamic writer without normalized ELF imports",
                    ),
                },
            )),
            (
                false,
                ExecutableImageEmissionRequest::Direct {
                    subsystem,
                    code_signature_identifier,
                },
            ) => emit_direct_executable_image_signed(
                artifact,
                subsystem,
                code_signature_identifier.as_deref(),
            )
            .map(RequestedExecutableImage::Direct)
            .map_err(|diagnostic| Box::new(RequestedExecutableImageError::Direct(diagnostic))),
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ObjectContainer {
        pub psi: TerminalPsiIdentity,
        pub output: ObjectContainerOutput,
    }

    pub fn can_emit_executable_image(target: NativeTarget) -> bool {
        target.pointer_size == 8
            && target.pointer_alignment == 8
            && matches!(
                (target.object_format, target.architecture),
                (ObjectFormat::Elf, Architecture::Aarch64)
                    | (ObjectFormat::Elf, Architecture::X86_64)
                    | (ObjectFormat::MachO, Architecture::Aarch64)
                    | (ObjectFormat::MachO, Architecture::X86_64)
                    | (ObjectFormat::Coff, Architecture::X86_64)
            )
    }

    pub(crate) fn emit_direct_executable_image_signed(
        artifact: &ObjectArtifact,
        subsystem: u16,
        code_signature_identifier: Option<&str>,
    ) -> Result<ExecutableImage, Diagnostic> {
        if !can_emit_executable_image(artifact.target) {
            return Err(Diagnostic::error(format!(
                "cannot emit terminal-Psi executable image for {:?}",
                artifact.target
            )));
        }
        let prepared_entry = crate::entries::hosted_unit_entry::prepare(artifact)?;
        let prepared =
            crate::entries::hosted_unit_entry::prepared_parts(artifact, prepared_entry.as_ref());
        let mut image = image::build_final_image(FinalImageInput {
            target: artifact.target,
            object: prepared.object,
            relocations: prepared.relocations,
            text_bytes: prepared.text_bytes,
            data_bytes: artifact.data_bytes(),
        });
        crate::image::entry_effects::attach(artifact, &mut image, prepared.entry_shim.as_ref())?;
        let final_image_symbol_digest = image::final_image_symbol_digest(&image);
        let output = match artifact.target.object_format {
            ObjectFormat::Elf => image_elf::emit_elf_executable(image),
            ObjectFormat::MachO => match code_signature_identifier {
                Some(identifier) => image_macho::emit_macho_executable_signed(image, identifier),
                None => image_macho::emit_macho_executable(image),
            },
            ObjectFormat::Coff => image_pe::emit_pe_x86_64_executable(image, subsystem),
        }?;
        let mut output = emitted_direct_executable_output(output);
        let text_validation = validate_terminal_image(
            artifact,
            prepared.object,
            prepared.relocations,
            prepared.text_bytes,
            prepared.entry_shim.clone(),
            &output,
        )?;
        output.compiler_function_validation =
            crate::object::function_fragments::reporting::summarize(
                artifact,
                &output,
                &text_validation,
            )?;
        output.compiler_text_validation = Some(text_validation);
        Ok(ExecutableImage {
            psi: artifact.psi,
            target: artifact.target,
            subsystem: matches!(artifact.target.object_format, ObjectFormat::Coff)
                .then_some(subsystem),
            functions: artifact.functions.clone(),
            private_functions: artifact.private_functions.clone(),
            semantic_code_attribution: artifact.semantic_code_attribution.clone(),
            port_effects: artifact.port_effects.clone(),
            boundary_settlements: artifact.boundary_settlements.clone(),
            foreign_calls: artifact.foreign_calls.clone(),
            final_image_symbol_digest,
            output,
        })
    }

    pub fn validate_direct_executable_image(
        artifact: &ObjectArtifact,
        image: &ExecutableImage,
    ) -> Result<(), Diagnostic> {
        validate_direct_executable_image_output(artifact, image, image.output())
    }

    pub fn validate_direct_executable_image_output(
        artifact: &ObjectArtifact,
        image: &ExecutableImage,
        output: &EmittedImageOutput,
    ) -> Result<(), Diagnostic> {
        crate::object::function_fragments::replay::validate(artifact)?;
        if artifact.psi() != image.psi()
            || artifact.target() != image.target()
            || artifact.functions() != image.functions()
            || artifact.private_functions() != image.private_functions()
            || artifact.semantic_code_attribution() != image.semantic_code_attribution()
            || artifact.port_effects() != image.port_effects()
            || artifact.boundary_settlements() != image.boundary_settlements()
            || !crate::object::object_artifact::image_foreign_calls_match_object(
                artifact,
                image.foreign_calls(),
            )
        {
            return Err(Diagnostic::error(
                "terminal object and executable image have different semantic or evidence identity",
            ));
        }
        let prepared_entry = crate::entries::hosted_unit_entry::prepare(artifact)?;
        let prepared =
            crate::entries::hosted_unit_entry::prepared_parts(artifact, prepared_entry.as_ref());
        let mut replayed_final_image = image::build_final_image(FinalImageInput {
            target: artifact.target(),
            object: prepared.object,
            relocations: prepared.relocations,
            text_bytes: prepared.text_bytes,
            data_bytes: artifact.data_bytes(),
        });
        crate::image::entry_effects::attach(
            artifact,
            &mut replayed_final_image,
            prepared.entry_shim.as_ref(),
        )?;
        if image.final_image_symbol_digest
            != image::final_image_symbol_digest(&replayed_final_image)
        {
            return Err(Diagnostic::error(
                "terminal executable image symbol evidence does not match its exact object entry/data-symbol table",
            ));
        }
        let recomputed = validate_terminal_image(
            artifact,
            prepared.object,
            prepared.relocations,
            prepared.text_bytes,
            prepared.entry_shim.clone(),
            output,
        )?;
        if let Some(shim) = prepared.entry_shim.as_ref()
            && shim.surface == crate::entries::hosted_unit_entry::EntrySurface::ProvisionedUnit
        {
            crate::entries::hosted_unit_entry::validate_unit_shim(
                shim.clone(),
                artifact,
                prepared.object,
                prepared.text_bytes,
                prepared.relocations,
                output,
            )?;
        }
        let function_validation =
            crate::object::function_fragments::reporting::summarize(artifact, output, &recomputed)?;
        if output.compiler_text_validation != Some(recomputed) {
            return Err(Diagnostic::error(
                "terminal executable image retained stale final-text validation evidence",
            ));
        }
        if output.compiler_function_validation != function_validation {
            return Err(Diagnostic::error(
                "terminal executable image retained stale compiler-function validation evidence",
            ));
        }
        Ok(())
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ExecutableImage {
        psi: TerminalPsiIdentity,
        target: NativeTarget,
        subsystem: Option<u16>,
        functions: Vec<ObjectFunction>,
        private_functions: Vec<ObjectCompilerPrivateFunction>,
        semantic_code_attribution: Vec<ObjectCodeAttribution>,
        port_effects: Vec<ObjectPortEffect>,
        boundary_settlements: Vec<ObjectBoundarySettlement>,
        foreign_calls: Vec<crate::ObjectForeignCall>,
        final_image_symbol_digest: image::FinalImageSymbolDigest,
        output: EmittedImageOutput,
    }

    impl ExecutableImage {
        pub const fn psi(&self) -> TerminalPsiIdentity {
            self.psi
        }

        pub const fn target(&self) -> NativeTarget {
            self.target
        }

        pub const fn subsystem(&self) -> Option<u16> {
            self.subsystem
        }

        pub const fn output(&self) -> &EmittedImageOutput {
            &self.output
        }

        pub fn boundary_settlements(&self) -> &[ObjectBoundarySettlement] {
            &self.boundary_settlements
        }

        pub fn foreign_calls(&self) -> &[crate::ObjectForeignCall] {
            &self.foreign_calls
        }

        pub fn functions(&self) -> &[ObjectFunction] {
            &self.functions
        }

        pub fn private_functions(&self) -> &[ObjectCompilerPrivateFunction] {
            &self.private_functions
        }

        pub fn port_effects(&self) -> &[ObjectPortEffect] {
            &self.port_effects
        }

        pub fn semantic_code_attribution(&self) -> &[ObjectCodeAttribution] {
            &self.semantic_code_attribution
        }

        pub const fn final_image_symbol_digest(&self) -> image::FinalImageSymbolDigest {
            self.final_image_symbol_digest
        }
    }
}

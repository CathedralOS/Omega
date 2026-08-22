use crate::pipeline::compile_options::CompileOptions;
use crate::pipeline::compile_policy::ExecutableTcbInstallationAuthorization;
use crate::pipeline::stages::EmittedProgram;
use omega_artifacts::ArtifactWriter;
use omega_image_emission::{
    ExecutableImageInput, can_emit_executable_image, emit_checked_executable_image,
};
use omega_object_file::{ObjectContainerInput, emit_omega_object_container};
use psi_diagnostics::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExecutablePublicationEvidence {
    certificate_fingerprint: u64,
    callback_placement_identity_fingerprint: u64,
    inventory_fingerprint: u64,
    file_name: String,
    format: String,
    container_byte_count: usize,
    container_fingerprint: u64,
    final_text_byte_count: usize,
    final_text_fingerprint: u64,
    evidence_fingerprint: u64,
}

impl ExecutablePublicationEvidence {
    fn current(
        image: &omega_image::EmittedImageOutput,
        certificate: &omega_image::FinalFootprintCertificate,
    ) -> Result<Self, Diagnostic> {
        certificate.validate_identity()?;
        if image.compiler_text_validation != Some(certificate.compiler_text_validation)
            || image.compiler_function_validation != Some(certificate.compiler_function_validation)
            || image.callback_placement_identity_fingerprint
                != certificate.callback_placement_identity_fingerprint
            || image.compiler_entry_footprint_binding
                != certificate.compiler_entry_footprint_binding
            || image.executable_regions != certificate.inventory
        {
            return Err(Diagnostic::error(
                "executable publication image does not match its final footprint certificate",
            ));
        }
        omega_image::validate_placed_executable_region_inventory(
            &image.executable_regions,
            &image.final_text_bytes,
        )?;
        let mut evidence = Self {
            certificate_fingerprint: certificate.certificate_fingerprint,
            callback_placement_identity_fingerprint: certificate
                .callback_placement_identity_fingerprint,
            inventory_fingerprint: image.executable_regions.inventory_fingerprint,
            file_name: image.file_name.clone(),
            format: image.format.clone(),
            container_byte_count: image.bytes.len(),
            container_fingerprint: byte_fingerprint(&image.bytes),
            final_text_byte_count: image.final_text_bytes.len(),
            final_text_fingerprint: byte_fingerprint(&image.final_text_bytes),
            evidence_fingerprint: 0,
        };
        evidence.evidence_fingerprint = evidence.recomputed_fingerprint();
        Ok(evidence)
    }

    fn validate(
        &self,
        image: &omega_image::EmittedImageOutput,
        certificate: &omega_image::FinalFootprintCertificate,
    ) -> Result<(), Diagnostic> {
        let expected = Self::current(image, certificate)?;
        if *self != expected {
            return Err(Diagnostic::error(
                "executable publication evidence does not match the exact container candidate",
            ));
        }
        Ok(())
    }

    fn recomputed_fingerprint(&self) -> u64 {
        let mut hash = FNV_OFFSET;
        fingerprint_into(&mut hash, b"omega.executable-publication-evidence.v1");
        fingerprint_into(&mut hash, &self.certificate_fingerprint.to_le_bytes());
        fingerprint_into(
            &mut hash,
            &self.callback_placement_identity_fingerprint.to_le_bytes(),
        );
        fingerprint_into(&mut hash, &self.inventory_fingerprint.to_le_bytes());
        fingerprint_into(&mut hash, &(self.file_name.len() as u64).to_le_bytes());
        fingerprint_into(&mut hash, self.file_name.as_bytes());
        fingerprint_into(&mut hash, &(self.format.len() as u64).to_le_bytes());
        fingerprint_into(&mut hash, self.format.as_bytes());
        fingerprint_into(&mut hash, &(self.container_byte_count as u64).to_le_bytes());
        fingerprint_into(&mut hash, &self.container_fingerprint.to_le_bytes());
        fingerprint_into(
            &mut hash,
            &(self.final_text_byte_count as u64).to_le_bytes(),
        );
        fingerprint_into(&mut hash, &self.final_text_fingerprint.to_le_bytes());
        hash
    }
}

struct ValidatedExecutablePublication<'a> {
    image: &'a omega_image::EmittedImageOutput,
    certificate: &'a omega_image::FinalFootprintCertificate,
    evidence: ExecutablePublicationEvidence,
}

impl<'a> ValidatedExecutablePublication<'a> {
    fn new(
        image: &'a omega_image::EmittedImageOutput,
        certificate: &'a omega_image::FinalFootprintCertificate,
    ) -> Result<Self, Diagnostic> {
        let evidence = ExecutablePublicationEvidence::current(image, certificate)?;
        evidence.validate(image, certificate)?;
        Ok(Self {
            image,
            certificate,
            evidence,
        })
    }

    fn validate_identity(&self) -> Result<(), Diagnostic> {
        self.evidence.validate(self.image, self.certificate)
    }

    fn bytes(&self) -> &[u8] {
        &self.image.bytes
    }

    fn file_name(&self) -> &str {
        &self.image.file_name
    }

    fn certificate(&self) -> &omega_image::FinalFootprintCertificate {
        self.certificate
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstalledExecutablePublicationEvidence {
    destination: super::ExecutablePublicationDestination,
    publication_evidence_fingerprint: u64,
    callback_placement_identity_fingerprint: u64,
    output_path: std::path::PathBuf,
    container_byte_count: usize,
    container_fingerprint: u64,
    evidence_fingerprint: u64,
}

impl InstalledExecutablePublicationEvidence {
    fn current(
        destination: super::ExecutablePublicationDestination,
        publication: &ValidatedExecutablePublication<'_>,
        output_path: &std::path::Path,
    ) -> Result<Self, Diagnostic> {
        publication.validate_identity()?;
        validate_executable_installation_path(publication, output_path)?;
        let mut evidence = Self {
            destination,
            publication_evidence_fingerprint: publication.evidence.evidence_fingerprint,
            callback_placement_identity_fingerprint: publication
                .evidence
                .callback_placement_identity_fingerprint,
            output_path: output_path.to_path_buf(),
            container_byte_count: publication.bytes().len(),
            container_fingerprint: byte_fingerprint(publication.bytes()),
            evidence_fingerprint: 0,
        };
        evidence.evidence_fingerprint = evidence.recomputed_fingerprint();
        Ok(evidence)
    }

    fn validate(
        &self,
        publication: &ValidatedExecutablePublication<'_>,
        output_path: &std::path::Path,
    ) -> Result<(), Diagnostic> {
        if *self != Self::current(self.destination, publication, output_path)? {
            return Err(Diagnostic::error(
                "installed executable receipt does not match the sealed publication",
            ));
        }
        Ok(())
    }

    fn recomputed_fingerprint(&self) -> u64 {
        super::compile_report::executable_installation_evidence_fingerprint(
            self.destination,
            self.publication_evidence_fingerprint,
            self.callback_placement_identity_fingerprint,
            &self.output_path,
            self.container_byte_count,
            self.container_fingerprint,
        )
    }

    fn retained_receipt(
        &self,
        publication: &ValidatedExecutablePublication<'_>,
    ) -> Result<super::ExecutablePublicationReceipt, Diagnostic> {
        self.validate(publication, &self.output_path)?;
        if let Err(diagnostic) =
            validate_published_executable_bytes(&self.output_path, publication.bytes())
        {
            let _ = std::fs::remove_file(&self.output_path);
            return Err(diagnostic);
        }
        Ok(super::ExecutablePublicationReceipt::new(
            self.destination,
            self.output_path.clone(),
            publication.certificate.certificate_fingerprint,
            publication
                .certificate
                .callback_placement_identity_fingerprint,
            publication.certificate.boundary_contract_fingerprint,
            publication.certificate.inventory.inventory_fingerprint,
            publication
                .certificate
                .compiler_text_validation
                .derivation_fingerprint,
            publication
                .certificate
                .compiler_function_validation
                .evidence_fingerprint(),
            publication.evidence.evidence_fingerprint,
            self.container_byte_count,
            self.container_fingerprint,
            self.evidence_fingerprint,
        ))
    }
}

pub(super) struct WrittenOutput {
    path: std::path::PathBuf,
    kind: super::CompileOutputKind,
    executable_publication: Option<super::ExecutablePublicationReceipt>,
    app_bundle_publication: Option<super::ExecutablePublicationReceipt>,
}

impl WrittenOutput {
    fn checked(
        root_path: &std::path::Path,
        path: std::path::PathBuf,
        kind: super::CompileOutputKind,
        executable_publication: Option<super::ExecutablePublicationReceipt>,
        app_bundle_publication: Option<super::ExecutablePublicationReceipt>,
    ) -> Result<Self, Diagnostic> {
        let path_matches_kind = match kind {
            super::CompileOutputKind::NativeExecutable => {
                executable_publication.as_ref().is_some_and(|receipt| {
                    receipt.destination() == super::ExecutablePublicationDestination::FlatOutput
                        && receipt.output_path() == path
                        && receipt.has_consistent_installation_identity()
                        && super::compile_report::executable_publication_pair_matches(
                            root_path,
                            receipt,
                            app_bundle_publication.as_ref(),
                        )
                })
            }
            super::CompileOutputKind::ObjectContainer => {
                executable_publication.is_none() && app_bundle_publication.is_none()
            }
            super::CompileOutputKind::CheckOnly => false,
        };
        if !path_matches_kind {
            return Err(Diagnostic::error(
                "written output path does not match its exact publication receipt",
            ));
        }
        Ok(Self {
            path,
            kind,
            executable_publication,
            app_bundle_publication,
        })
    }

    pub(super) fn into_report_parts(
        self,
    ) -> (
        std::path::PathBuf,
        super::CompileOutputKind,
        Option<super::ExecutablePublicationReceipt>,
        Option<super::ExecutablePublicationReceipt>,
    ) {
        (
            self.path,
            self.kind,
            self.executable_publication,
            self.app_bundle_publication,
        )
    }
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

fn byte_fingerprint(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    fingerprint_into(&mut hash, bytes);
    hash
}

fn fingerprint_into(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

pub(super) fn write_output(
    options: &CompileOptions,
    executable_tcb_authorization: &ExecutableTcbInstallationAuthorization,
    emitted: EmittedProgram,
    footprints: &omega_target_operations::BoundaryFootprintPlan,
    emit_auxiliary_artifacts: bool,
    validate_before_publication: impl FnOnce(
        Option<&omega_image::EmittedImageOutput>,
    ) -> Result<(), Vec<Diagnostic>>,
) -> Result<WrittenOutput, Vec<Diagnostic>> {
    executable_tcb_authorization.authorize_installation();
    let build_dir = options.build_dir();
    std::fs::create_dir_all(&build_dir).map_err(io_diagnostic)?;

    if can_emit_executable_image(emitted.target) {
        let mut image = emit_checked_executable_image(
            ExecutableImageInput {
                target: emitted.target,
                callback_placement_identity_fingerprint: emitted
                    .callback_placement_identity_fingerprint,
                object: &emitted.object,
                relocations: &emitted.relocations,
                encoded_machine_code: &emitted.encoded_machine_code,
                encoded_machine_semantics: &emitted.encoded_machine_semantics,
                text_bytes: &emitted.text_bytes,
                data_bytes: &emitted.data_bytes,
                subsystem: emitted.subsystem,
            },
            emitted.planned_text_bytes,
        )
        .map_err(|diagnostic| vec![diagnostic])?;

        if footprints.boundary_contract_fingerprint.is_some() {
            let final_region_binding_fingerprint = image
                .compiler_function_validation
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "checked executable image omitted compiler-function validation evidence",
                    )]
                })?
                .final_region_binding_fingerprint;
            let entry_binding = image
                .compiler_entry_region_binding
                .as_ref()
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "checked executable image omitted exact compiler-entry region custody",
                    )]
                })?;
            let footprint_binding = omega_image::bind_compiler_entry_footprint(
                &mut image.executable_regions,
                entry_binding,
                final_region_binding_fingerprint,
                footprints.composed_evidence(),
            )
            .map_err(|diagnostic| vec![diagnostic])?;
            image.compiler_entry_footprint_binding = Some(footprint_binding);
        }

        // Entry-specific evidence must join the exact relocated image before
        // any executable, bundle, or auxiliary final-image artifact becomes
        // externally visible.
        validate_before_publication(Some(&image))?;

        let compiler_text_validation = image.compiler_text_validation.ok_or_else(|| {
            vec![Diagnostic::error(
                "checked executable image omitted compiler-text validation evidence",
            )]
        })?;
        let compiler_function_validation = image.compiler_function_validation.ok_or_else(|| {
            vec![Diagnostic::error(
                "checked executable image omitted compiler-function validation evidence",
            )]
        })?;
        let final_footprint_certificate = build_final_footprint_certificate(
            footprints,
            emitted.callback_placement_identity_fingerprint,
            compiler_text_validation,
            compiler_function_validation,
            image.compiler_entry_footprint_binding,
            &image.executable_regions,
        )?;
        let publication = ValidatedExecutablePublication::new(&image, &final_footprint_certificate)
            .map_err(|diagnostic| vec![diagnostic])?;
        publication
            .validate_identity()
            .map_err(|diagnostic| vec![diagnostic])?;
        let output_path = build_dir.join(publication.file_name());
        let installation = write_validated_executable_output_file(
            super::ExecutablePublicationDestination::FlatOutput,
            &output_path,
            &publication,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        installation
            .validate(&publication, &output_path)
            .map_err(|diagnostic| vec![diagnostic])?;
        let executable_publication = installation
            .retained_receipt(&publication)
            .map_err(|diagnostic| vec![diagnostic])?;
        write_executable_region_inventory(
            options,
            publication.certificate(),
            emit_auxiliary_artifacts,
        )?;

        // The GUI-subsystem translation for Mach-O: PE stamps Subsystem 2 into
        // the image header so Windows never attaches a console box; macOS has no
        // header equivalent — a bare executable double-clicked in Finder routes
        // through Terminal. The equivalent is an `.app` bundle, so lay one out
        // beside the flat binary (which stays, for tests and terminal runs).
        // The embedded ad-hoc signature is content-hashed, so the copied
        // executable stays valid inside the bundle.
        let app_bundle_publication = if emitted.target.object_format
            == omega_target::ObjectFormat::MachO
            && emitted.subsystem == GUI_SUBSYSTEM
        {
            publication
                .validate_identity()
                .map_err(|diagnostic| vec![diagnostic])?;
            Some(write_macos_app_bundle(options, &output_path, &publication)?)
        } else {
            None
        };
        return WrittenOutput::checked(
            &options.root_path,
            output_path,
            super::CompileOutputKind::NativeExecutable,
            Some(executable_publication),
            app_bundle_publication,
        )
        .map_err(|diagnostic| vec![diagnostic]);
    }

    let object_container = emit_omega_object_container(ObjectContainerInput {
        target: emitted.target,
        object: &emitted.object,
        relocations: &emitted.relocations,
        text_bytes: &emitted.text_bytes,
        data_bytes: &emitted.data_bytes,
    });
    validate_before_publication(None)?;
    let output_path = build_dir.join(&object_container.file_name);
    write_output_file(&output_path, &object_container.bytes, false)
        .map_err(|diagnostic| vec![diagnostic])?;
    WrittenOutput::checked(
        &options.root_path,
        output_path,
        super::CompileOutputKind::ObjectContainer,
        None,
        None,
    )
    .map_err(|diagnostic| vec![diagnostic])
}

fn build_final_footprint_certificate(
    footprints: &omega_target_operations::BoundaryFootprintPlan,
    callback_placement_identity_fingerprint: u64,
    compiler_text_validation: omega_image::CompilerTextValidationEvidence,
    compiler_function_validation: omega_image::CompilerFunctionValidationEvidence,
    compiler_entry_footprint_binding: Option<omega_image::CompilerEntryFootprintBindingEvidence>,
    inventory: &omega_image::PlacedExecutableRegionInventory,
) -> Result<omega_image::FinalFootprintCertificate, Vec<Diagnostic>> {
    let implementation_evidence_fingerprint = footprints.composed_evidence().evidence_fingerprint();
    let certificate = omega_image::FinalFootprintCertificate::current(
        footprints.boundary_contract_fingerprint,
        implementation_evidence_fingerprint,
        footprints.fragments.len(),
        callback_placement_identity_fingerprint,
        compiler_text_validation,
        compiler_function_validation,
        compiler_entry_footprint_binding,
        inventory.clone(),
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    certificate
        .validate_identity()
        .map_err(|diagnostic| vec![diagnostic])?;
    Ok(certificate)
}

fn write_executable_region_inventory(
    options: &CompileOptions,
    certificate: &omega_image::FinalFootprintCertificate,
    emit_auxiliary_artifacts: bool,
) -> Result<(), Vec<Diagnostic>> {
    fn push_string(output: &mut String, value: &str) {
        output.push('"');
        for character in value.chars() {
            match character {
                '"' => output.push_str("\\\""),
                '\\' => output.push_str("\\\\"),
                '\n' => output.push_str("\\n"),
                '\r' => output.push_str("\\r"),
                '\t' => output.push_str("\\t"),
                character => output.push(character),
            }
        }
        output.push('"');
    }

    fn push_footprint(
        output: &mut String,
        footprint: Option<&omega_calling_conventions::StateFootprintEvidence>,
    ) {
        let Some(footprint) = footprint else {
            output.push_str("null");
            return;
        };
        output.push_str("{\"fingerprint\": ");
        push_string(
            output,
            &format!("0x{:016x}", footprint.evidence_fingerprint()),
        );
        output.push_str(&format!(
            ", \"machine_state_bits\": {}, \"registers\": [",
            footprint.machine_state().bits()
        ));
        for (index, register) in footprint.registers().as_slice().iter().enumerate() {
            if index > 0 {
                output.push_str(", ");
            }
            push_string(output, &format!("{register:?}"));
        }
        output.push_str("]}");
    }

    fn push_classes(output: &mut String, classes: &[omega_image::FinalFootprintClass]) {
        output.push('[');
        for (index, class) in classes.iter().enumerate() {
            if index > 0 {
                output.push_str(", ");
            }
            push_string(output, class.name());
        }
        output.push(']');
    }

    if !emit_auxiliary_artifacts {
        return Ok(());
    }
    let coverage = &certificate.coverage;
    let inventory = &certificate.inventory;
    let implementation_evidence_fingerprint = certificate.implementation_evidence_fingerprint;
    let mut json = format!(
        "{{\n  \"certificate_marker\": \"{}\",\n  \"certificate_fingerprint\": \"0x{:016x}\",\n  \"coverage_fingerprint\": \"0x{:016x}\",\n  \"placement_stage\": \"final_image\",\n  \"enumeration_complete\": {},\n  \"region_enumeration_complete\": {},\n  \"footprint_enumeration_complete\": {},\n",
        certificate.marker,
        certificate.certificate_fingerprint,
        certificate.coverage_fingerprint,
        coverage.enumeration_complete,
        coverage.region_enumeration_complete,
        coverage.footprint_enumeration_complete,
    );
    for (name, classes) in [
        ("covered_classes", &coverage.covered_classes),
        (
            "absent_by_construction_classes",
            &coverage.absent_by_construction_classes,
        ),
        (
            "final_byte_validated_classes",
            &coverage.final_byte_validated_classes,
        ),
        ("missing_classes", &coverage.missing_classes),
    ] {
        json.push_str("  \"");
        json.push_str(name);
        json.push_str("\": ");
        push_classes(&mut json, classes);
        json.push_str(",\n");
    }
    json.push_str("  \"boundary_contract_fingerprint\": ");
    if let Some(fingerprint) = certificate.boundary_contract_fingerprint {
        push_string(&mut json, &format!("0x{fingerprint:016x}"));
    } else {
        json.push_str("null");
    }
    json.push_str(&format!(
        ",\n  \"implementation_evidence_fingerprint\": \"0x{implementation_evidence_fingerprint:016x}\",\n  \"implementation_fragment_count\": {},\n  \"callback_placement_identity_fingerprint\": \"0x{:016x}\",\n  \"compiler_text_validation\": {{\"encoded_text_fingerprint\": \"0x{:016x}\", \"final_compiler_text_fingerprint\": \"0x{:016x}\", \"relocation_envelope_fingerprint\": \"0x{:016x}\", \"checked_instruction_validation_fingerprint\": \"0x{:016x}\", \"checked_instruction_footprint_fingerprint\": \"0x{:016x}\", \"derivation_fingerprint\": \"0x{:016x}\", \"text_relocation_count\": {}, \"checked_instruction_validation_count\": {}}},\n  \"compiler_function_validation\": {{\"evidence_fingerprint\": \"0x{:016x}\", \"validation_fingerprint\": \"0x{:016x}\", \"final_region_binding_fingerprint\": \"0x{:016x}\", \"function_count\": {}, \"instruction_count\": {}, \"zero_width_instruction_count\": {}, \"checked_assembly_instruction_count\": {}, \"fixed_mechanics_instruction_count\": {}, \"fixed_mechanics_validation_fingerprint\": \"0x{:016x}\", \"fixed_mechanics_boundary_contract_fingerprint\": \"0x{:016x}\", \"fixed_mechanics_footprint_fingerprint\": \"0x{:016x}\", \"body_specification_instruction_count\": {}, \"body_specification_validation_fingerprint\": \"0x{:016x}\", \"body_specification_boundary_contract_fingerprint\": \"0x{:016x}\", \"body_specification_footprint_fingerprint\": \"0x{:016x}\", \"composed_footprint_fingerprint\": \"0x{:016x}\"}},\n  \"inventory_fingerprint\": \"0x{:016x}\",\n  \"boundary_placement_binding_fingerprint\": \"0x{:016x}\",\n",
        certificate.implementation_fragment_count,
        certificate.callback_placement_identity_fingerprint,
        certificate.compiler_text_validation.encoded_text_fingerprint,
        certificate.compiler_text_validation.final_compiler_text_fingerprint,
        certificate.compiler_text_validation.relocation_envelope_fingerprint,
        certificate
            .compiler_text_validation
            .checked_instruction_validation_fingerprint,
        certificate
            .compiler_text_validation
            .checked_instruction_footprint_fingerprint,
        certificate.compiler_text_validation.derivation_fingerprint,
        certificate.compiler_text_validation.text_relocation_count,
        certificate
            .compiler_text_validation
            .checked_instruction_validation_count,
        certificate
            .compiler_function_validation
            .evidence_fingerprint(),
        certificate
            .compiler_function_validation
            .validation_fingerprint,
        certificate
            .compiler_function_validation
            .final_region_binding_fingerprint,
        certificate.compiler_function_validation.function_count,
        certificate.compiler_function_validation.instruction_count,
        certificate
            .compiler_function_validation
            .zero_width_instruction_count,
        certificate
            .compiler_function_validation
            .checked_assembly_instruction_count,
        certificate
            .compiler_function_validation
            .fixed_mechanics_instruction_count,
        certificate
            .compiler_function_validation
            .fixed_mechanics_validation_fingerprint,
        certificate
            .compiler_function_validation
            .fixed_mechanics_boundary_contract_fingerprint,
        certificate
            .compiler_function_validation
            .fixed_mechanics_footprint_fingerprint,
        certificate
            .compiler_function_validation
            .body_specification_instruction_count,
        certificate
            .compiler_function_validation
            .body_specification_validation_fingerprint,
        certificate
            .compiler_function_validation
            .body_specification_boundary_contract_fingerprint,
        certificate
            .compiler_function_validation
            .body_specification_footprint_fingerprint,
        certificate
            .compiler_function_validation
            .composed_footprint_fingerprint,
        inventory.inventory_fingerprint,
        certificate.boundary_placement_binding_fingerprint,
    ));
    json.push_str("  \"entry_footprint_binding\": ");
    if let Some(binding) = certificate.compiler_entry_footprint_binding {
        json.push_str(&format!(
            "{{\"evidence_fingerprint\": \"0x{:016x}\", \"entry_region_evidence_fingerprint\": \"0x{:016x}\", \"final_region_binding_fingerprint\": \"0x{:016x}\", \"prior_inventory_fingerprint\": \"0x{:016x}\", \"footprint_fingerprint\": \"0x{:016x}\", \"resulting_inventory_fingerprint\": \"0x{:016x}\"}}",
            binding.evidence_fingerprint,
            binding.entry_region_evidence_fingerprint,
            binding.final_region_binding_fingerprint,
            binding.prior_inventory_fingerprint,
            binding.footprint_fingerprint,
            binding.resulting_inventory_fingerprint,
        ));
    } else {
        json.push_str("null");
    }
    json.push_str(",\n");
    json.push_str(&format!(
        "  \"text_address\": \"0x{:016x}\",\n  \"text_byte_count\": {},\n  \"text_fingerprint\": \"0x{:016x}\",\n  \"regions\": [",
        inventory.text_address, inventory.text_byte_count, inventory.text_fingerprint
    ));
    for (index, region) in inventory.regions.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\"origin\": ");
        push_string(
            &mut json,
            match region.origin {
                omega_image::FinalExecutableRegionOrigin::CompilerFunction => "compiler_function",
                omega_image::FinalExecutableRegionOrigin::ImportThunk => "import_thunk",
            },
        );
        json.push_str(", \"symbol\": ");
        push_string(&mut json, &region.symbol);
        json.push_str(&format!(
            ", \"section_offset\": {}, \"address\": \"0x{:016x}\", \"byte_count\": {}, \"byte_fingerprint\": \"0x{:016x}\", \"footprint\": ",
            region.section_offset, region.address, region.byte_count, region.byte_fingerprint
        ));
        push_footprint(&mut json, region.footprint.as_ref());
        json.push('}');
    }
    if !inventory.regions.is_empty() {
        json.push('\n');
        json.push_str("  ");
    }
    json.push_str("],\n  \"unclassified_gaps\": [");
    for (index, gap) in inventory.unclassified_gaps.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "\n    {{\"section_offset\": {}, \"address\": \"0x{:016x}\", \"byte_count\": {}, \"byte_fingerprint\": \"0x{:016x}\"}}",
            gap.section_offset, gap.address, gap.byte_count, gap.byte_fingerprint
        ));
    }
    if !inventory.unclassified_gaps.is_empty() {
        json.push('\n');
        json.push_str("  ");
    }
    json.push_str("]\n}\n");

    ArtifactWriter::new(&options.build_dir())
        .and_then(|writer| writer.write_text("13_executable_regions.json", &json))
        .map_err(|diagnostic| vec![diagnostic])
}

fn io_diagnostic(error: std::io::Error) -> Vec<Diagnostic> {
    vec![Diagnostic::error(error.to_string())]
}

/// PE optional-header Subsystem word for a GUI program (`Subsystem::Gui`;
/// console is 3). Shared meaning across targets: the PE writer stamps it, the
/// Mach-O path translates it into an `.app` bundle.
const GUI_SUBSYSTEM: u16 = 2;

/// Lays out `build/<name>.app/Contents/{Info.plist,PkgInfo,MacOS/<exe>}` so a
/// Finder launch runs the program as a real windowed app (no Terminal). `<name>`
/// is the project directory name (e.g. `window_demo`).
fn write_macos_app_bundle(
    options: &CompileOptions,
    flat_output_path: &std::path::Path,
    publication: &ValidatedExecutablePublication<'_>,
) -> Result<super::ExecutablePublicationReceipt, Vec<Diagnostic>> {
    let executable_name = publication.file_name();
    // Keep the plist honest without an XML escaper: path characters that are
    // XML-significant or exotic collapse to '-' in the shared canonical name.
    let app_name = super::compile_report::macos_app_bundle_name(&options.root_path);
    let bundle_identifier: String = app_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect();

    let executable_path = super::compile_report::expected_macos_app_bundle_executable_path(
        &options.root_path,
        flat_output_path,
    )
    .ok_or_else(|| {
        vec![Diagnostic::error(
            "flat executable path cannot derive a canonical macOS app-bundle destination",
        )]
    })?;
    let macos_dir = executable_path.parent().ok_or_else(|| {
        vec![Diagnostic::error(
            "canonical macOS app-bundle executable has no MacOS directory",
        )]
    })?;
    let contents_dir = macos_dir.parent().ok_or_else(|| {
        vec![Diagnostic::error(
            "canonical macOS app-bundle executable has no Contents directory",
        )]
    })?;
    std::fs::create_dir_all(&macos_dir).map_err(io_diagnostic)?;

    let info_plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleExecutable</key>
	<string>{executable_name}</string>
	<key>CFBundleIdentifier</key>
	<string>org.omega-lang.{bundle_identifier}</string>
	<key>CFBundleName</key>
	<string>{app_name}</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>NSHighResolutionCapable</key>
	<true/>
</dict>
</plist>
"#
    );
    std::fs::write(contents_dir.join("Info.plist"), info_plist).map_err(io_diagnostic)?;
    std::fs::write(contents_dir.join("PkgInfo"), b"APPL????").map_err(io_diagnostic)?;
    let installation = write_validated_executable_output_file(
        super::ExecutablePublicationDestination::MacOsAppBundle,
        &executable_path,
        publication,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    installation
        .validate(publication, &executable_path)
        .map_err(|diagnostic| vec![diagnostic])?;
    installation
        .retained_receipt(publication)
        .map_err(|diagnostic| vec![diagnostic])
}

fn write_validated_executable_output_file(
    destination: super::ExecutablePublicationDestination,
    output_path: &std::path::Path,
    publication: &ValidatedExecutablePublication<'_>,
) -> Result<InstalledExecutablePublicationEvidence, Diagnostic> {
    publication.validate_identity()?;
    validate_executable_installation_path(publication, output_path)?;
    write_output_file_with_staged_validation(
        output_path,
        publication.bytes(),
        true,
        |staged_path| validate_staged_executable_bytes(staged_path, publication.bytes()),
    )?;
    if let Err(diagnostic) = validate_published_executable_bytes(output_path, publication.bytes()) {
        let _ = std::fs::remove_file(output_path);
        return Err(diagnostic);
    }
    let installed =
        InstalledExecutablePublicationEvidence::current(destination, publication, output_path)?;
    installed.validate(publication, output_path)?;
    Ok(installed)
}

fn validate_executable_installation_path(
    publication: &ValidatedExecutablePublication<'_>,
    output_path: &std::path::Path,
) -> Result<(), Diagnostic> {
    if output_path.file_name() != Some(std::ffi::OsStr::new(publication.file_name())) {
        return Err(Diagnostic::error(
            "executable installation path does not retain the sealed output name",
        ));
    }
    Ok(())
}

fn validate_staged_executable_bytes(
    staged_path: &std::path::Path,
    expected_bytes: &[u8],
) -> Result<(), Diagnostic> {
    let staged_bytes = std::fs::read(staged_path).map_err(|error| {
        Diagnostic::error(format!(
            "failed to replay staged executable {}: {error}",
            staged_path.display()
        ))
    })?;
    if staged_bytes != expected_bytes {
        return Err(Diagnostic::error(
            "staged executable bytes do not match the sealed publication",
        ));
    }
    Ok(())
}

fn validate_published_executable_bytes(
    output_path: &std::path::Path,
    expected_bytes: &[u8],
) -> Result<(), Diagnostic> {
    let published_bytes = std::fs::read(output_path).map_err(|error| {
        Diagnostic::error(format!(
            "failed to replay published executable {}: {error}",
            output_path.display()
        ))
    })?;
    if published_bytes != expected_bytes {
        return Err(Diagnostic::error(
            "published executable bytes do not match the sealed publication",
        ));
    }
    Ok(())
}

fn write_output_file(
    output_path: &std::path::Path,
    bytes: &[u8],
    executable: bool,
) -> Result<(), Diagnostic> {
    write_output_file_with_staged_validation(output_path, bytes, executable, |_| Ok(()))
}

fn write_output_file_with_staged_validation(
    output_path: &std::path::Path,
    bytes: &[u8],
    executable: bool,
    validate_staged: impl FnOnce(&std::path::Path) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    let temp_path = output_path.with_file_name(format!(
        ".{}.{}.tmp",
        output_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("omega-output"),
        std::process::id()
    ));
    let _ = std::fs::remove_file(&temp_path);
    std::fs::write(&temp_path, bytes).map_err(|error| {
        Diagnostic::error(format!("failed to write {}: {error}", temp_path.display()))
    })?;

    if let Err(diagnostic) = validate_staged(&temp_path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(diagnostic);
    }

    if executable {
        mark_executable_if_needed(&temp_path)?;
    }

    match std::fs::rename(&temp_path, output_path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = std::fs::remove_file(&temp_path);
            Err(Diagnostic::error(format!(
                "failed to install {}: {error}",
                output_path.display()
            )))
        }
    }
}

#[cfg(unix)]
fn mark_executable_if_needed(path: &std::path::Path) -> Result<(), Diagnostic> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .map_err(|error| Diagnostic::error(format!("failed to read {}: {error}", path.display())))?
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).map_err(|error| {
        Diagnostic::error(format!(
            "failed to mark {} executable: {error}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn mark_executable_if_needed(_path: &std::path::Path) -> Result<(), Diagnostic> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::ExecutablePublicationDestination;
    use super::{
        ExecutablePublicationEvidence, FNV_OFFSET, ValidatedExecutablePublication, WrittenOutput,
        build_final_footprint_certificate, fingerprint_into, validate_published_executable_bytes,
        validate_staged_executable_bytes, write_validated_executable_output_file,
    };

    fn compiler_text_validation() -> omega_image::CompilerTextValidationEvidence {
        omega_image::CompilerTextValidationEvidence {
            encoded_text_fingerprint: 1,
            final_compiler_text_fingerprint: 2,
            relocation_envelope_fingerprint: 3,
            checked_instruction_validation_fingerprint: 4,
            checked_instruction_footprint_fingerprint: 5,
            derivation_fingerprint: 6,
            text_relocation_count: 0,
            checked_instruction_validation_count: 0,
        }
    }

    fn compiler_function_validation() -> omega_image::CompilerFunctionValidationEvidence {
        omega_image::CompilerFunctionValidationEvidence {
            function_count: 0,
            instruction_count: 0,
            zero_width_instruction_count: 0,
            checked_assembly_instruction_count: 0,
            fixed_mechanics_instruction_count: 0,
            fixed_mechanics_validation_fingerprint: 0,
            fixed_mechanics_boundary_contract_fingerprint: 0,
            fixed_mechanics_footprint_fingerprint: 0,
            body_specification_instruction_count: 0,
            body_specification_validation_fingerprint: 0,
            body_specification_boundary_contract_fingerprint: 0,
            body_specification_footprint_fingerprint: 0,
            composed_footprint_fingerprint: 0,
            final_region_binding_fingerprint: 7,
            validation_fingerprint: 8,
        }
    }

    fn inventory() -> omega_image::PlacedExecutableRegionInventory {
        let text_fingerprint = FNV_OFFSET;
        let mut inventory_fingerprint = FNV_OFFSET;
        fingerprint_into(&mut inventory_fingerprint, &0x1000u64.to_le_bytes());
        fingerprint_into(&mut inventory_fingerprint, &0u64.to_le_bytes());
        fingerprint_into(&mut inventory_fingerprint, &text_fingerprint.to_le_bytes());
        omega_image::PlacedExecutableRegionInventory {
            text_address: 0x1000,
            text_byte_count: 0,
            text_fingerprint,
            inventory_fingerprint,
            regions: Vec::new(),
            unclassified_gaps: Vec::new(),
        }
    }

    fn image() -> omega_image::EmittedImageOutput {
        omega_image::EmittedImageOutput {
            bytes: vec![0x7f, b'O', b'M', b'G'],
            final_text_bytes: Vec::new(),
            callback_placement_identity_fingerprint: 0,
            file_name: "main".into(),
            format: "test".into(),
            kind: omega_image::ImageOutputKind::DirectExecutable,
            text_bytes: 0,
            data_bytes: 0,
            bss_bytes: 0,
            symbols: 0,
            relocations: 0,
            final_image_symbols: 0,
            final_image_imports: 0,
            final_image_relocations: 0,
            executable_regions: inventory(),
            compiler_text_validation: Some(compiler_text_validation()),
            compiler_function_validation: Some(compiler_function_validation()),
            compiler_entry_region_binding: None,
            compiler_entry_footprint_binding: None,
        }
    }

    #[test]
    fn final_footprint_gate_rejects_missing_boundary_mutation_custody() {
        let without_boundary = omega_target_operations::BoundaryFootprintPlan::default();
        build_final_footprint_certificate(
            &without_boundary,
            0,
            compiler_text_validation(),
            compiler_function_validation(),
            None,
            &inventory(),
        )
        .expect("a boundary-free image needs no entry mutation receipt");

        let with_boundary = omega_target_operations::BoundaryFootprintPlan {
            boundary_contract_fingerprint: Some(11),
            ..omega_target_operations::BoundaryFootprintPlan::default()
        };
        assert!(
            build_final_footprint_certificate(
                &with_boundary,
                0,
                compiler_text_validation(),
                compiler_function_validation(),
                None,
                &inventory(),
            )
            .is_err()
        );
    }

    #[test]
    fn executable_publication_evidence_rejects_container_candidate_drift() {
        let image = image();
        let certificate = build_final_footprint_certificate(
            &omega_target_operations::BoundaryFootprintPlan::default(),
            0,
            compiler_text_validation(),
            compiler_function_validation(),
            None,
            &image.executable_regions,
        )
        .expect("certificate");
        let evidence = ExecutablePublicationEvidence::current(&image, &certificate)
            .expect("exact publication evidence");
        evidence
            .validate(&image, &certificate)
            .expect("unchanged candidate");

        let substituted_callback_identity = build_final_footprint_certificate(
            &omega_target_operations::BoundaryFootprintPlan::default(),
            99,
            compiler_text_validation(),
            compiler_function_validation(),
            None,
            &image.executable_regions,
        )
        .expect("independently valid certificate with substituted callback identity");
        assert!(
            ExecutablePublicationEvidence::current(&image, &substituted_callback_identity).is_err()
        );

        let mut changed_bytes = image.clone();
        changed_bytes.bytes[3] ^= 1;
        assert!(evidence.validate(&changed_bytes, &certificate).is_err());

        let mut changed_name = image.clone();
        changed_name.file_name = "redirected".into();
        assert!(evidence.validate(&changed_name, &certificate).is_err());

        let mut changed_format = image;
        changed_format.format = "redirected".into();
        assert!(evidence.validate(&changed_format, &certificate).is_err());
    }

    #[test]
    fn executable_installation_replays_exact_staged_bytes_before_publication() {
        let image = image();
        let certificate = build_final_footprint_certificate(
            &omega_target_operations::BoundaryFootprintPlan::default(),
            0,
            compiler_text_validation(),
            compiler_function_validation(),
            None,
            &image.executable_regions,
        )
        .expect("certificate");
        let publication =
            ValidatedExecutablePublication::new(&image, &certificate).expect("publication");
        let directory = std::env::temp_dir().join(format!(
            "omega-executable-publication-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("test directory");

        let staged = directory.join("staged");
        std::fs::write(&staged, [0x7f, b'O', b'M', b'F']).expect("drifted candidate");
        assert!(validate_staged_executable_bytes(&staged, publication.bytes()).is_err());

        let drifted_destination = directory.join(publication.file_name());
        std::fs::write(&drifted_destination, [0x7f, b'O', b'M', b'F'])
            .expect("drifted destination");
        assert!(
            validate_published_executable_bytes(&drifted_destination, publication.bytes()).is_err()
        );

        let output = drifted_destination;
        let receipt = write_validated_executable_output_file(
            ExecutablePublicationDestination::FlatOutput,
            &output,
            &publication,
        )
        .expect("validated installation");
        receipt
            .validate(&publication, &output)
            .expect("exact receipt");
        std::fs::write(&output, [0x7f, b'O', b'M', b'F'])
            .expect("post-installation destination drift");
        assert!(receipt.retained_receipt(&publication).is_err());
        assert!(!output.exists());

        let receipt = write_validated_executable_output_file(
            ExecutablePublicationDestination::FlatOutput,
            &output,
            &publication,
        )
        .expect("revalidated installation");
        let retained = receipt
            .retained_receipt(&publication)
            .expect("compile report receipt");
        assert_eq!(retained.output_path(), output);
        assert_eq!(
            retained.destination(),
            ExecutablePublicationDestination::FlatOutput
        );
        assert_eq!(
            retained.certificate_fingerprint(),
            certificate.certificate_fingerprint
        );
        assert_eq!(
            retained.callback_placement_identity_fingerprint(),
            certificate.callback_placement_identity_fingerprint
        );
        assert_eq!(
            retained.boundary_contract_fingerprint(),
            certificate.boundary_contract_fingerprint
        );
        assert_eq!(
            retained.inventory_fingerprint(),
            image.executable_regions.inventory_fingerprint
        );
        assert_eq!(
            retained.compiler_text_validation_fingerprint(),
            certificate.compiler_text_validation.derivation_fingerprint
        );
        assert_eq!(
            retained.compiler_function_validation_fingerprint(),
            certificate
                .compiler_function_validation
                .evidence_fingerprint()
        );
        assert_eq!(retained.container_byte_count(), image.bytes.len());
        assert_eq!(
            retained.container_fingerprint(),
            super::byte_fingerprint(&image.bytes)
        );
        assert!(retained.has_consistent_installation_identity());
        let root_path = directory.join("Main/main.omg");
        assert!(
            WrittenOutput::checked(
                &root_path,
                output.clone(),
                super::super::CompileOutputKind::NativeExecutable,
                Some(retained.clone()),
                None,
            )
            .is_ok()
        );
        assert!(
            WrittenOutput::checked(
                &root_path,
                directory.join("redirected"),
                super::super::CompileOutputKind::NativeExecutable,
                Some(retained.clone()),
                None,
            )
            .is_err()
        );
        assert!(
            WrittenOutput::checked(
                &root_path,
                output.clone(),
                super::super::CompileOutputKind::NativeExecutable,
                None,
                None,
            )
            .is_err()
        );
        assert!(
            WrittenOutput::checked(
                &root_path,
                output.clone(),
                super::super::CompileOutputKind::ObjectContainer,
                Some(retained.clone()),
                None,
            )
            .is_err()
        );
        assert!(
            WrittenOutput::checked(
                &root_path,
                output.clone(),
                super::super::CompileOutputKind::ObjectContainer,
                None,
                None,
            )
            .is_ok()
        );
        assert!(
            WrittenOutput::checked(
                &root_path,
                output.clone(),
                super::super::CompileOutputKind::CheckOnly,
                None,
                None,
            )
            .is_err()
        );

        let bundle_directory = directory.join("Main.app/Contents/MacOS");
        std::fs::create_dir_all(&bundle_directory).expect("bundle directory");
        let bundle_output = bundle_directory.join(publication.file_name());
        let bundle_installation = write_validated_executable_output_file(
            ExecutablePublicationDestination::MacOsAppBundle,
            &bundle_output,
            &publication,
        )
        .expect("validated bundle installation");
        let bundle_retained = bundle_installation
            .retained_receipt(&publication)
            .expect("bundle compile report receipt");
        assert_ne!(bundle_retained.output_path(), retained.output_path());
        assert_eq!(
            bundle_retained.destination(),
            ExecutablePublicationDestination::MacOsAppBundle
        );
        assert_eq!(
            bundle_retained.publication_evidence_fingerprint(),
            retained.publication_evidence_fingerprint()
        );
        assert_eq!(
            bundle_retained.container_fingerprint(),
            retained.container_fingerprint()
        );
        assert!(bundle_retained.has_consistent_installation_identity());
        assert!(
            WrittenOutput::checked(
                &root_path,
                output.clone(),
                super::super::CompileOutputKind::NativeExecutable,
                Some(retained.clone()),
                Some(bundle_retained.clone()),
            )
            .is_ok()
        );
        assert!(
            WrittenOutput::checked(
                &directory.join("Other/main.omg"),
                output.clone(),
                super::super::CompileOutputKind::NativeExecutable,
                Some(retained.clone()),
                Some(bundle_retained.clone()),
            )
            .is_err()
        );
        assert!(
            WrittenOutput::checked(
                &root_path,
                bundle_output,
                super::super::CompileOutputKind::NativeExecutable,
                Some(bundle_retained.clone()),
                None,
            )
            .is_err()
        );
        assert_eq!(
            std::fs::read(&output).expect("installed bytes"),
            image.bytes
        );
        assert!(
            receipt
                .validate(&publication, &directory.join("redirected"))
                .is_err()
        );

        std::fs::remove_dir_all(&directory).expect("remove test directory");
    }
}

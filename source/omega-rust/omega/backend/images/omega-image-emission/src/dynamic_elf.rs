//! Production-emitter custody for already-admitted dynamic ELF bytes.
//!
//! This bridge consumes only the exact final-byte carrier produced by the ELF
//! owner and independently rejoins it to the source-free object artifact.  It
//! deliberately does not construct loader inputs, publish bytes, create an
//! installation receipt, or grant execution authority.

use omega_image::{
    EmittedImageOutput, FinalImageInput, emitted_direct_executable_output,
    final_image_symbol_digest,
};
use omega_image_elf::ValidatedElfDynamicExecutable;
use omega_target::{Architecture, ObjectFormat};
use psi_diagnostics::Diagnostic;

use crate::ObjectArtifact;
use crate::final_image_validation::validate_terminal_dynamic_elf_image;

/// Exact admitted dynamic ELF bytes after production-emitter reconciliation.
///
/// This is intentionally not [`crate::ExecutableImage`], so existing
/// installation and installed-artifact APIs cannot mistake final-byte custody
/// for publication or execution authority.
///
/// ```compile_fail
/// use omega_image_emission::{
///     DynamicElfImageEmission, build_installation_record,
/// };
/// use psi_core::ProfileDecisionId;
///
/// fn cannot_install(
///     emission: &DynamicElfImageEmission,
///     profile: ProfileDecisionId,
/// ) {
///     let _ = build_installation_record(emission, profile);
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

    pub fn into_admitted(self) -> ValidatedElfDynamicExecutable {
        self.admitted
    }
}

/// Rejected production-emitter reconciliation with the admitted owner intact.
#[derive(Debug)]
#[must_use = "dynamic ELF emission rejection retains admitted byte custody"]
pub struct DynamicElfImageEmissionError {
    admitted: ValidatedElfDynamicExecutable,
    diagnostic: Diagnostic,
}

impl DynamicElfImageEmissionError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfDynamicExecutable, Diagnostic) {
        (self.admitted, self.diagnostic)
    }
}

impl std::fmt::Display for DynamicElfImageEmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for DynamicElfImageEmissionError {}

/// Join exact admitted dynamic ELF bytes to the production image-output
/// surface without granting publication or execution authority.
pub fn emit_admitted_dynamic_elf_image(
    artifact: &ObjectArtifact,
    admitted: ValidatedElfDynamicExecutable,
) -> Result<DynamicElfImageEmission, Box<DynamicElfImageEmissionError>> {
    let output = match derive_output(artifact, &admitted) {
        Ok(output) => output,
        Err(diagnostic) => {
            return Err(Box::new(DynamicElfImageEmissionError {
                admitted,
                diagnostic,
            }));
        }
    };
    let emission = DynamicElfImageEmission { admitted, output };
    if let Err(diagnostic) = validate_dynamic_elf_image_emission(artifact, &emission) {
        return Err(Box::new(DynamicElfImageEmissionError {
            admitted: emission.admitted,
            diagnostic,
        }));
    }
    Ok(emission)
}

/// Independently replay one admitted dynamic ELF production-emission join.
pub fn validate_dynamic_elf_image_emission(
    artifact: &ObjectArtifact,
    emission: &DynamicElfImageEmission,
) -> Result<(), Diagnostic> {
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
    super::ranked_u32_countdown::replay_ranked_u32_countdown_final_image(artifact)?;
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

    let mut replayed_image = omega_image::build_final_image(FinalImageInput {
        target,
        object: artifact.object(),
        relocations: artifact.relocations(),
        text_bytes: artifact.text_bytes(),
        data_bytes: &[],
    });
    let symbol_digest = final_image_symbol_digest(&replayed_image);
    let mut output = emitted_direct_executable_output(admitted.output().clone());
    let validation = validate_terminal_dynamic_elf_image(artifact, &output)?;
    replayed_image.memory.text = output.final_text_bytes.clone();
    if replayed_image != *admitted.image()
        || symbol_digest != final_image_symbol_digest(admitted.image())
    {
        return Err(Diagnostic::error(
            "admitted dynamic ELF final image does not replay from the exact object artifact",
        ));
    }
    output.compiler_text_validation = Some(validation);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ObjectFunction;
    use omega_image::FinalImageInput;
    use omega_image_elf::{
        admit_elf_dynamic_executable, apply_elf_dynamic_address_fixups,
        apply_elf_procedure_linkage_fixups, apply_elf_section_header_placements,
        assemble_elf_dynamic_file, plan_elf_dynamic_link_inputs, plan_elf_dynamic_load_layout,
        plan_elf_dynamic_section_descriptors, plan_elf_dynamic_section_roster,
        plan_elf_dynamic_sections, plan_elf_dynamic_table_section_descriptor,
        plan_elf_dynamic_tags, plan_elf_indexed_section_payloads,
        plan_elf_procedure_linkage_relocations, plan_elf_procedure_linkage_section_descriptors,
        plan_elf_procedure_linkage_templates, plan_elf_relative_section_payload_layout,
        plan_elf_section_name_table, serialize_elf_dynamic_file_envelope,
        serialize_elf_dynamic_sections, serialize_elf_dynamic_table,
        serialize_elf_section_header_table,
    };
    use omega_object_file::{
        NormalizedImportPlan, ObjectPlan, RelocationKind, RelocationOrigin, RelocationPlan,
        RelocationRecord, SectionKind, SectionPlan, SymbolKind, SymbolPlan, SymbolSection,
    };
    use omega_target::{
        ForeignLocatorCandidate, TargetProfile, normalize_elf_interpreter_plan,
        normalize_foreign_locator,
    };
    use omega_target_operations::TerminalPsiProvenance;
    use psi_core::MachineId;
    use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    fn artifact(target: TargetProfile) -> ObjectArtifact {
        let native = target.native_target();
        let mut object = ObjectPlan::with_capacity(native, 1, 3);
        object.layout.sections.insert(SectionPlan {
            kind: SectionKind::Text,
            size: 64,
            alignment: 16,
        });
        let entry = object.layout.symbols.insert(SymbolPlan {
            name: "_start".to_owned(),
            section: SymbolSection::Section(SectionKind::Text),
            offset: 0,
            size: 64,
            kind: SymbolKind::Function,
            import_library: String::new(),
        });
        object.layout.entry_symbol = entry;

        let mut imported = Vec::new();
        for (index, name) in [b"alpha_call".as_slice(), b"beta_call"]
            .into_iter()
            .enumerate()
        {
            let symbol = object.layout.symbols.insert(SymbolPlan {
                name: format!("__omega_dynamic_import_{index}"),
                section: SymbolSection::None,
                offset: 0,
                size: 0,
                kind: SymbolKind::Import,
                import_library: String::new(),
            });
            object.layout.normalized_imports.push(NormalizedImportPlan {
                symbol,
                locator: normalize_foreign_locator(
                    ForeignLocatorCandidate::ElfVersioned {
                        object: b"libproduction-emitter.so".to_vec(),
                        symbol: name.to_vec(),
                        version: b"PRODUCTION_EMITTER_1".to_vec(),
                    },
                    target,
                )
                .unwrap(),
            });
            imported.push(symbol);
        }

        let mut text = vec![0; 64];
        let mut relocations = RelocationPlan::with_record_capacity(native, 3);
        for (ordinal, (instruction_offset, symbol_handle)) in
            [(0, imported[0]), (8, imported[0]), (16, imported[1])]
                .into_iter()
                .enumerate()
        {
            let (offset, kind) = match target {
                TargetProfile::LinuxX64 => {
                    text[instruction_offset] = 0xe8;
                    (instruction_offset + 1, RelocationKind::X86_64Relative32)
                }
                TargetProfile::LinuxArm64 => {
                    text[instruction_offset..instruction_offset + 4]
                        .copy_from_slice(&[0, 0, 0, 0x94]);
                    (instruction_offset, RelocationKind::Aarch64Branch26)
                }
                _ => unreachable!(),
            };
            relocations.push_record(RelocationRecord {
                origin: RelocationOrigin::Instruction {
                    function_symbol_handle: entry,
                    selected_instruction_index: ordinal as u32,
                },
                section: SectionKind::Text,
                offset,
                byte_width: 4,
                symbol_handle,
                addend: 0,
                kind,
            });
        }

        let machine = MachineId::new(1).unwrap();
        ObjectArtifact {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([0x5a; 32]),
            },
            target: native,
            entry: machine,
            object,
            relocations,
            text_bytes: text,
            functions: vec![ObjectFunction {
                machine,
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: Vec::new(),
                },
                symbol: entry,
                text_offset: 0,
                byte_count: 64,
                unit_stack: None,
                scalar_stack: None,
                unit_call_stacks: Vec::new(),
                scalar_call_stacks: Vec::new(),
                internal_unit_calls: Vec::new(),
                unit_parameters: Vec::new(),
                unit_parameter_homes: Vec::new(),
                unit_affine_cleanup: None,
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                ranked_u32_countdown: None,
                structural_return: None,
            }],
            fuel_attribution: Vec::new(),
            port_effects: Vec::new(),
            boundary_settlements: Vec::new(),
        }
    }

    fn admitted(artifact: &ObjectArtifact, target: TargetProfile) -> ValidatedElfDynamicExecutable {
        let image = omega_image::build_final_image(FinalImageInput {
            target: artifact.target(),
            object: artifact.object(),
            relocations: artifact.relocations(),
            text_bytes: artifact.text_bytes(),
            data_bytes: &[],
        });
        let path = match target {
            TargetProfile::LinuxX64 => b"/lib64/ld-linux-x86-64.so.2".as_slice(),
            TargetProfile::LinuxArm64 => b"/lib/ld-linux-aarch64.so.1".as_slice(),
            _ => unreachable!(),
        };
        let interpreter = normalize_elf_interpreter_plan(path.to_vec(), target).unwrap();
        let inputs = plan_elf_dynamic_link_inputs(image, interpreter).unwrap();
        let sections = plan_elf_dynamic_sections(inputs).unwrap();
        let payloads = serialize_elf_dynamic_sections(sections).unwrap();
        let descriptors = plan_elf_dynamic_section_descriptors(payloads).unwrap();
        let linkage = plan_elf_procedure_linkage_relocations(descriptors).unwrap();
        let templates = plan_elf_procedure_linkage_templates(linkage).unwrap();
        let descriptors = plan_elf_procedure_linkage_section_descriptors(templates).unwrap();
        let tags = plan_elf_dynamic_tags(descriptors).unwrap();
        let dynamic = serialize_elf_dynamic_table(tags).unwrap();
        let descriptor = plan_elf_dynamic_table_section_descriptor(dynamic).unwrap();
        let names = plan_elf_section_name_table(descriptor).unwrap();
        let roster = plan_elf_dynamic_section_roster(names).unwrap();
        let headers = serialize_elf_section_header_table(roster).unwrap();
        let payloads = plan_elf_indexed_section_payloads(headers).unwrap();
        let relative = plan_elf_relative_section_payload_layout(payloads).unwrap();
        let load = plan_elf_dynamic_load_layout(relative).unwrap();
        let placed = apply_elf_section_header_placements(load).unwrap();
        let resolved = apply_elf_dynamic_address_fixups(placed).unwrap();
        let envelope = serialize_elf_dynamic_file_envelope(resolved).unwrap();
        let linkage = apply_elf_procedure_linkage_fixups(envelope).unwrap();
        let assembled = assemble_elf_dynamic_file(linkage).unwrap();
        admit_elf_dynamic_executable(assembled).unwrap()
    }

    #[test]
    fn both_linux_targets_rejoin_exact_admitted_bytes_without_installation_authority() {
        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let artifact = artifact(target);
            let admitted = admitted(&artifact, target);
            let exact_bytes = admitted.output().bytes.clone();
            let exact_fingerprint = admitted.assembled_file_compatibility_fingerprint();

            let emission = emit_admitted_dynamic_elf_image(&artifact, admitted).unwrap();
            validate_dynamic_elf_image_emission(&artifact, &emission).unwrap();
            assert_eq!(emission.output().bytes, exact_bytes);
            assert_eq!(
                emission
                    .admitted()
                    .assembled_file_compatibility_fingerprint(),
                exact_fingerprint,
            );
            assert_eq!(emission.output().final_image_imports, 2);
            assert!(emission.output().format.contains("dynamic-executable"));
        }
    }

    #[test]
    fn target_drift_rejects_with_exact_admitted_custody() {
        let source = artifact(TargetProfile::LinuxX64);
        let admitted = admitted(&source, TargetProfile::LinuxX64);
        let exact_bytes = admitted.output().bytes.clone();
        let exact_fingerprint = admitted.assembled_file_compatibility_fingerprint();
        let wrong_artifact = artifact(TargetProfile::LinuxArm64);

        let error = emit_admitted_dynamic_elf_image(&wrong_artifact, admitted).unwrap_err();
        let (admitted, diagnostic) = error.into_parts();
        assert!(diagnostic.to_string().contains("target does not match"));
        assert_eq!(admitted.output().bytes, exact_bytes);
        assert_eq!(
            admitted.assembled_file_compatibility_fingerprint(),
            exact_fingerprint,
        );
    }

    #[test]
    fn artifact_drift_rejects_with_exact_admitted_custody() {
        let mut artifact = artifact(TargetProfile::LinuxX64);
        let admitted = admitted(&artifact, TargetProfile::LinuxX64);
        let exact_bytes = admitted.output().bytes.clone();
        let exact_fingerprint = admitted.assembled_file_compatibility_fingerprint();
        artifact.text_bytes[32] ^= 1;

        let error = emit_admitted_dynamic_elf_image(&artifact, admitted).unwrap_err();
        let (admitted, diagnostic) = error.into_parts();
        assert!(
            diagnostic
                .to_string()
                .contains("changed outside its declared relocation field")
        );
        assert_eq!(admitted.output().bytes, exact_bytes);
        assert_eq!(
            admitted.assembled_file_compatibility_fingerprint(),
            exact_fingerprint,
        );
    }

    #[test]
    fn import_drift_rejects_with_exact_admitted_custody() {
        let mut artifact = artifact(TargetProfile::LinuxX64);
        let admitted = admitted(&artifact, TargetProfile::LinuxX64);
        let exact_bytes = admitted.output().bytes.clone();
        let exact_fingerprint = admitted.assembled_file_compatibility_fingerprint();
        artifact.object.layout.normalized_imports[0].locator = normalize_foreign_locator(
            ForeignLocatorCandidate::ElfVersioned {
                object: b"libproduction-emitter.so".to_vec(),
                symbol: b"mutated_call".to_vec(),
                version: b"PRODUCTION_EMITTER_1".to_vec(),
            },
            TargetProfile::LinuxX64,
        )
        .unwrap();

        let error = emit_admitted_dynamic_elf_image(&artifact, admitted).unwrap_err();
        let (admitted, diagnostic) = error.into_parts();
        assert!(
            diagnostic
                .to_string()
                .contains("does not replay from the exact object artifact")
        );
        assert_eq!(admitted.output().bytes, exact_bytes);
        assert_eq!(
            admitted.assembled_file_compatibility_fingerprint(),
            exact_fingerprint,
        );
    }

    #[test]
    fn independent_replay_rejects_production_output_mutation() {
        let artifact = artifact(TargetProfile::LinuxX64);
        let admitted = admitted(&artifact, TargetProfile::LinuxX64);
        let mut emission = emit_admitted_dynamic_elf_image(&artifact, admitted).unwrap();
        emission.output.bytes[0] ^= 1;
        assert!(validate_dynamic_elf_image_emission(&artifact, &emission).is_err());
    }
}

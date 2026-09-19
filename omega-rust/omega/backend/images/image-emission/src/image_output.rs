//! Object-container and direct executable-image output dispatch.
//!
//! This module selects the target writer, retains the sealed terminal-Psi
//! image carrier, and invokes independent final-image replay before returning.
//! It does not construct machine code or installation authority.

use diagnostics::Diagnostic;
use image::{EmittedImageOutput, FinalImageInput, emitted_direct_executable_output};
use object_file::{ObjectContainerInput, ObjectContainerOutput, emit_omega_object_container};
use target::{Architecture, NativeTarget, ObjectFormat};
use terminal_psi::TerminalPsiIdentity;

use super::final_image_validation::validate_terminal_image;
use super::{
    LinuxX86ScalarExitShim, ObjectArtifact, ObjectBoundarySettlement, ObjectCodeAttribution,
    ObjectCompilerPrivateFunction, ObjectFunction, ObjectPortEffect,
};
use crate::hosted_unit_entry::{
    LINUX_X86_SCALAR_EXIT_SHIM_BYTES, SCALAR_CALL_REFERENCE_FINGERPRINT,
};

fn validate_x86_scalar_fma_provider(artifact: &ObjectArtifact) -> Result<(), Diagnostic> {
    let fragments = artifact
        .functions
        .iter()
        .flat_map(|function| function.x86_scalar_fma.iter())
        .collect::<Vec<_>>();
    if fragments.is_empty() {
        if artifact.x86_scalar_fma_provider.is_some() {
            return Err(Diagnostic::error(
                "x86 scalar FMA provider admission has no retained instruction custody",
            ));
        }
        return Ok(());
    }
    let provider = artifact.x86_scalar_fma_provider.ok_or_else(|| {
        Diagnostic::error(
            "x86 scalar FMA feature requirements have no admitted executable provider",
        )
    })?;
    if !provider.has_canonical_identity()
        || Some(provider.profile()) != artifact.x86_feature_profile
        || provider.profile().native_target() != artifact.target
    {
        return Err(Diagnostic::error(
            "x86 scalar FMA executable provider admission does not match its exact object target",
        ));
    }
    for fragment in fragments {
        let slot = match fragment.format {
            machine_code::X86ScalarFmaFormat::Binary32 => target::X86ScalarFmaSlot::Binary32,
            machine_code::X86ScalarFmaFormat::Binary64 => target::X86ScalarFmaSlot::Binary64,
        };
        if !provider.admits(fragment.requirement, slot) {
            return Err(Diagnostic::error(format!(
                "x86 scalar FMA provider does not admit exact generic slot `{}`",
                slot.requirement_identity(),
            )));
        }
    }
    Ok(())
}

pub fn emit_object_container(artifact: &ObjectArtifact) -> ObjectContainer {
    ObjectContainer {
        psi: artifact.psi,
        output: emit_omega_object_container(ObjectContainerInput {
            target: artifact.target,
            object: &artifact.object,
            relocations: &artifact.relocations,
            text_bytes: &artifact.text_bytes,
            data_bytes: artifact.data_bytes(),
        }),
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
                | (ObjectFormat::Coff, Architecture::X86_64)
        )
}

/// Emit and validate one direct executable image.
///
/// The direct lane admits typed internal calls and exactly retained imports
/// supported by its target writer. Final-text mutation outside their
/// architecture-specific immediate bits, unaccounted imports or thunks,
/// overlapping/missing function spans, and unclassified executable bytes are
/// hard failures.
///
/// This entry carries no bound signing identity: Mach-O output falls back to
/// the validated executable leaf as its ad-hoc CodeDirectory label. The
/// build-bound identity enters only through
/// [`crate::ExecutableImageEmissionRequest::Direct`]
/// (wiki/spec/build/macos_application.md).
pub fn emit_executable_image(
    artifact: &ObjectArtifact,
    subsystem: u16,
) -> Result<ExecutableImage, Diagnostic> {
    emit_executable_image_signed(artifact, subsystem, None)
}

/// `code_signature_identifier` is the build-bound Mach-O signing identity
/// carried by the image request. It is written verbatim into the emitted
/// CodeDirectory and participates in `LC_CODE_SIGNATURE` extent planning, so
/// it is fixed before any load command is emitted.
pub(crate) fn emit_executable_image_signed(
    artifact: &ObjectArtifact,
    subsystem: u16,
    code_signature_identifier: Option<&str>,
) -> Result<ExecutableImage, Diagnostic> {
    super::function_fragments::replay::validate(artifact)?;
    validate_x86_scalar_fma_provider(artifact)?;
    if !can_emit_executable_image(artifact.target) {
        return Err(Diagnostic::error(format!(
            "cannot emit terminal-Psi executable image for {:?}",
            artifact.target
        )));
    }
    let prepared_entry = super::hosted_unit_entry::prepare(artifact)?;
    let (object, text_bytes, relocations, entry_shim) = prepared_entry.as_ref().map_or(
        (
            &artifact.object,
            artifact.text_bytes.as_slice(),
            &artifact.relocations,
            None,
        ),
        |prepared| {
            (
                &prepared.object,
                prepared.text.as_slice(),
                &prepared.relocations,
                Some(prepared.shim),
            )
        },
    );
    let image = image::build_final_image(FinalImageInput {
        target: artifact.target,
        object,
        relocations,
        text_bytes,
        data_bytes: artifact.data_bytes(),
    });
    let final_image_symbol_digest = image::final_image_symbol_digest(&image);
    let output = match (artifact.target.object_format, artifact.target.architecture) {
        (ObjectFormat::Elf, Architecture::Aarch64) => image_elf::emit_elf_aarch64_executable(image),
        (ObjectFormat::Elf, Architecture::X86_64) => image_elf::emit_elf_x86_64_executable(image),
        (ObjectFormat::MachO, Architecture::Aarch64) => match code_signature_identifier {
            Some(identifier) => {
                image_macho::emit_macho_aarch64_executable_signed(image, identifier)
            }
            None => image_macho::emit_macho_aarch64_executable(image),
        },
        (ObjectFormat::Coff, Architecture::X86_64) => {
            image_pe::emit_pe_x86_64_executable(image, subsystem)
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "cannot emit terminal-Psi executable image for {:?}",
                artifact.target
            )));
        }
    }?;
    let mut output = emitted_direct_executable_output(output);
    let text_validation = validate_terminal_image(
        artifact,
        object,
        relocations,
        text_bytes,
        entry_shim,
        &output,
    )?;
    output.compiler_function_validation =
        super::function_fragments::reporting::summarize(artifact, &output, &text_validation)?;
    output.compiler_text_validation = Some(text_validation);
    Ok(ExecutableImage {
        psi: artifact.psi,
        target: artifact.target,
        x86_scalar_fma_provider: artifact.x86_scalar_fma_provider,
        subsystem: matches!(artifact.target.object_format, ObjectFormat::Coff).then_some(subsystem),
        functions: artifact.functions.clone(),
        private_functions: artifact.private_functions.clone(),
        dynamic_conformance_tables: artifact.dynamic_conformance_tables.clone(),
        forwarded_dynamic_descriptor_adapters: artifact
            .forwarded_dynamic_descriptor_adapters
            .clone(),
        forwarded_dynamic_descriptor_tables: artifact.forwarded_dynamic_descriptor_tables.clone(),
        semantic_code_attribution: artifact.semantic_code_attribution.clone(),
        port_effects: artifact.port_effects.clone(),
        boundary_settlements: artifact.boundary_settlements.clone(),
        foreign_calls: artifact.foreign_calls.clone(),
        final_image_symbol_digest,
        output,
    })
}

/// Independently replay the complete object-to-executable-image join retained
/// by a source-free native artifact.
///
/// This repeats final-text relocation-envelope validation and requires the
/// recomputed evidence to equal the evidence sealed by image construction.
pub fn validate_executable_image(
    artifact: &ObjectArtifact,
    image: &ExecutableImage,
) -> Result<(), Diagnostic> {
    super::function_fragments::replay::validate(artifact)?;
    validate_x86_scalar_fma_provider(artifact)?;
    if artifact.psi() != image.psi()
        || artifact.target() != image.target()
        || artifact.x86_scalar_fma_provider() != image.x86_scalar_fma_provider()
        || artifact.functions() != image.functions()
        || artifact.private_functions() != image.private_functions()
        || artifact.dynamic_conformance_tables() != image.dynamic_conformance_tables()
        || artifact.forwarded_dynamic_descriptor_adapters()
            != image.forwarded_dynamic_descriptor_adapters()
        || artifact.forwarded_dynamic_descriptor_tables()
            != image.forwarded_dynamic_descriptor_tables()
        || artifact.semantic_code_attribution() != image.semantic_code_attribution()
        || artifact.port_effects() != image.port_effects()
        || artifact.boundary_settlements() != image.boundary_settlements()
        || !crate::object_artifact::image_foreign_calls_match_object(
            artifact,
            image.foreign_calls(),
        )
    {
        return Err(Diagnostic::error(
            "terminal object and executable image have different semantic or evidence identity",
        ));
    }
    let prepared_entry = super::hosted_unit_entry::prepare(artifact)?;
    let (object, text_bytes, relocations, entry_shim) = prepared_entry.as_ref().map_or(
        (
            artifact.object(),
            artifact.text_bytes(),
            artifact.relocations(),
            None,
        ),
        |prepared| {
            (
                &prepared.object,
                prepared.text.as_slice(),
                &prepared.relocations,
                Some(prepared.shim),
            )
        },
    );
    let replayed_final_image = image::build_final_image(FinalImageInput {
        target: artifact.target(),
        object,
        relocations,
        text_bytes,
        data_bytes: artifact.data_bytes(),
    });
    if image.final_image_symbol_digest != image::final_image_symbol_digest(&replayed_final_image) {
        return Err(Diagnostic::error(
            "terminal executable image symbol evidence does not match its exact object entry/data-symbol table",
        ));
    }
    let recomputed = validate_terminal_image(
        artifact,
        object,
        relocations,
        text_bytes,
        entry_shim,
        image.output(),
    )?;
    let function_validation =
        super::function_fragments::reporting::summarize(artifact, image.output(), &recomputed)?;
    if image.output().compiler_text_validation != Some(recomputed) {
        return Err(Diagnostic::error(
            "terminal executable image retained stale final-text validation evidence",
        ));
    }
    if image.output().compiler_function_validation != function_validation {
        return Err(Diagnostic::error(
            "terminal executable image retained stale compiler-function validation evidence",
        ));
    }
    Ok(())
}

/// Emit the runnable Linux x86-64 image for the exact published proof-free i32
/// scalar-call reference.
///
/// This fixture/profile-specific API is deliberately not a general scalar
/// process adapter. `ObjectFunction` does not retain ordinary scalar
/// arity, and an unused entry parameter can produce byte-identical machine code.
/// Exact semantic-identity binding prevents such an entry from silently
/// acquiring zero-argument process-entry semantics.
pub fn emit_scalar_call_reference_linux_x86_64_image(
    artifact: &ObjectArtifact,
) -> Result<ScalarCallReferenceImage, Diagnostic> {
    if artifact.target != NativeTarget::linux_x64() {
        return Err(Diagnostic::error(format!(
            "Linux x86-64 scalar entry shim cannot target {:?}",
            artifact.target
        )));
    }
    if artifact.psi.vocabulary_marker != terminal_psi::VocabularyMarker::CURRENT
        || artifact.psi.program_fingerprint.as_bytes() != &SCALAR_CALL_REFERENCE_FINGERPRINT
    {
        return Err(Diagnostic::error(format!(
            "Linux x86-64 scalar-call reference image requires the exact published semantic identity; got {}:{}, expected {}:{:02x?}",
            artifact.psi.vocabulary_marker.get(),
            artifact.psi.program_fingerprint,
            terminal_psi::VocabularyMarker::CURRENT.get(),
            SCALAR_CALL_REFERENCE_FINGERPRINT,
        )));
    }
    let entry = artifact.entry_function();
    if entry.scalar_stack.is_none() || entry.bytes(artifact).last() != Some(&0xc3) {
        return Err(Diagnostic::error(format!(
            "terminal entry {} is not a completely accounted returning scalar function",
            artifact.entry
        )));
    }

    let mut object = artifact.object.clone();
    let mut relocations = artifact.relocations.clone();
    let mut text_bytes = artifact.text_bytes.clone();
    let text_offset = text_bytes.len();
    text_bytes.extend_from_slice(&LINUX_X86_SCALAR_EXIT_SHIM_BYTES);

    let text_section = object
        .layout
        .sections
        .iter()
        .find(|(_, section)| section.kind == object_file::SectionKind::Text)
        .map(|(handle, _)| handle)
        .ok_or_else(|| Diagnostic::error("terminal object has no text section"))?;
    object.layout.sections.get_mut(text_section).size = text_bytes.len();

    let symbol = object.layout.symbols.insert(object_file::SymbolPlan {
        name: "omega_terminal_linux_x86_64_scalar_exit_entry".into(),
        section: object_file::SymbolSection::Section(object_file::SectionKind::Text),
        offset: text_offset,
        size: LINUX_X86_SCALAR_EXIT_SHIM_BYTES.len(),
        kind: object_file::SymbolKind::Function,
        import_library: String::new(),
    });
    object.layout.entry_symbol = symbol;
    let relocation_offset = text_offset
        .checked_add(1)
        .ok_or_else(|| Diagnostic::error("terminal scalar entry relocation offset overflows"))?;
    relocations.push_record(object_file::RelocationRecord {
        origin: object_file::RelocationOrigin::Instruction {
            function_symbol_handle: symbol,
            selected_instruction_index: 0,
        },
        section: object_file::SectionKind::Text,
        offset: relocation_offset,
        byte_width: 4,
        symbol_handle: entry.symbol,
        addend: 0,
        kind: object_file::RelocationKind::X86_64Relative32,
    });
    let shim = LinuxX86ScalarExitShim {
        symbol,
        target_symbol: entry.symbol,
        text_offset,
        byte_count: LINUX_X86_SCALAR_EXIT_SHIM_BYTES.len(),
        relocation_offset,
    };

    let image = image::build_final_image(FinalImageInput {
        target: artifact.target,
        object: &object,
        relocations: &relocations,
        text_bytes: &text_bytes,
        data_bytes: artifact.data_bytes(),
    });
    let output = image_elf::emit_elf_x86_64_executable(image)?;
    let mut output = emitted_direct_executable_output(output);
    output.compiler_text_validation = Some(validate_terminal_image(
        artifact,
        &object,
        &relocations,
        &text_bytes,
        Some(super::hosted_unit_entry::EntryShim::LinuxScalar(shim)),
        &output,
    )?);
    Ok(ScalarCallReferenceImage {
        psi: artifact.psi,
        target: artifact.target,
        shim,
        output,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableImage {
    psi: TerminalPsiIdentity,
    target: NativeTarget,
    x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
    subsystem: Option<u16>,
    functions: Vec<ObjectFunction>,
    private_functions: Vec<ObjectCompilerPrivateFunction>,
    dynamic_conformance_tables: Vec<super::ObjectDynamicConformanceTable>,
    forwarded_dynamic_descriptor_adapters: Vec<super::ObjectForwardedDynamicDescriptorAdapter>,
    forwarded_dynamic_descriptor_tables: Vec<super::ObjectForwardedDynamicDescriptorTable>,
    semantic_code_attribution: Vec<ObjectCodeAttribution>,
    port_effects: Vec<ObjectPortEffect>,
    boundary_settlements: Vec<ObjectBoundarySettlement>,
    foreign_calls: Vec<super::ObjectForeignCall>,
    final_image_symbol_digest: image::FinalImageSymbolDigest,
    output: EmittedImageOutput,
}

impl ExecutableImage {
    #[cfg(any(test, feature = "test-support"))]
    pub fn output_mut_for_test(&mut self) -> &mut EmittedImageOutput {
        &mut self.output
    }

    pub const fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn x86_scalar_fma_provider(&self) -> Option<target::AdmittedX86ScalarFmaProvider> {
        self.x86_scalar_fma_provider
    }

    /// PE/COFF subsystem selected by the writer. Other formats carry no
    /// subsystem fact because the argument is not interpreted by their writer.
    pub const fn subsystem(&self) -> Option<u16> {
        self.subsystem
    }

    pub const fn output(&self) -> &EmittedImageOutput {
        &self.output
    }

    pub fn boundary_settlements(&self) -> &[ObjectBoundarySettlement] {
        &self.boundary_settlements
    }

    pub fn foreign_calls(&self) -> &[super::ObjectForeignCall] {
        &self.foreign_calls
    }

    /// Bind fragment-publication foreign-call custody onto a freshly emitted
    /// image.
    ///
    /// The fragment route seals `ObjectArtifact::foreign_calls` inside its
    /// replay surface, so normalized foreign calls projected after emission
    /// arrive through this binder rather than the constructor's clone. Every
    /// row must rejoin the image's own custody — an object function caller,
    /// a semantic operation owner attributed inside that function, and a
    /// `text_offset` inside one of that operation's attributed intervals —
    /// and owners must stay unique across the bound roster.
    pub fn bind_normalized_foreign_call_custody(
        &mut self,
        custody: Vec<super::ObjectForeignCall>,
    ) -> Result<(), Diagnostic> {
        for row in &custody {
            let Some(operation) = row.owner.operation() else {
                return Err(Diagnostic::error(
                    "image foreign call custody row has no semantic operation owner",
                ));
            };
            if !self
                .functions
                .iter()
                .any(|function| function.machine == row.machine)
            {
                return Err(Diagnostic::error(
                    "image foreign call custody names an absent object function",
                ));
            }
            if !self.semantic_code_attribution.iter().any(|attribution| {
                attribution.machine == row.machine
                    && attribution.attribution.site
                        == machine_code::SemanticCodeSite::Operation(operation)
                    && row.text_offset >= attribution.text_offset
                    && row.text_offset
                        < attribution
                            .text_offset
                            .saturating_add(attribution.attribution.byte_count)
            }) {
                return Err(Diagnostic::error(
                    "image foreign call custody does not rejoin an attributed call site",
                ));
            }
            if self
                .foreign_calls
                .iter()
                .chain(custody.iter())
                .filter(|existing| existing.machine == row.machine && existing.owner == row.owner)
                .count()
                != 1
            {
                return Err(Diagnostic::error(
                    "image foreign call custody owner is not unique",
                ));
            }
        }
        self.foreign_calls.extend(custody);
        Ok(())
    }

    pub fn functions(&self) -> &[ObjectFunction] {
        &self.functions
    }

    pub fn private_functions(&self) -> &[ObjectCompilerPrivateFunction] {
        &self.private_functions
    }

    pub fn dynamic_conformance_tables(&self) -> &[super::ObjectDynamicConformanceTable] {
        &self.dynamic_conformance_tables
    }

    pub fn forwarded_dynamic_descriptor_adapters(
        &self,
    ) -> &[super::ObjectForwardedDynamicDescriptorAdapter] {
        &self.forwarded_dynamic_descriptor_adapters
    }

    pub fn forwarded_dynamic_descriptor_tables(
        &self,
    ) -> &[super::ObjectForwardedDynamicDescriptorTable] {
        &self.forwarded_dynamic_descriptor_tables
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

/// Differential-only runnable image for the exact published scalar-call
/// reference. This deliberately is not `ExecutableImage`, so it cannot
/// be passed to installation-record or installed-artifact APIs that account
/// only semantic terminal functions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarCallReferenceImage {
    psi: TerminalPsiIdentity,
    target: NativeTarget,
    shim: LinuxX86ScalarExitShim,
    output: EmittedImageOutput,
}

impl ScalarCallReferenceImage {
    pub const fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn linux_x86_scalar_exit_shim(&self) -> LinuxX86ScalarExitShim {
        self.shim
    }

    pub const fn output(&self) -> &EmittedImageOutput {
        &self.output
    }
}

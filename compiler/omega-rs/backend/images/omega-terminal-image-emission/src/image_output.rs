//! Object-container and direct executable-image output dispatch.
//!
//! This module selects the target writer, retains the sealed terminal-Psi
//! image carrier, and invokes independent final-image replay before returning.
//! It does not construct machine code or installation authority.

use omega_image::{EmittedImageOutput, FinalImageInput, emitted_direct_executable_output};
use omega_object_file::{ObjectContainerInput, ObjectContainerOutput, emit_omega_object_container};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use psi_diagnostics::Diagnostic;
use psi_terminal::TerminalPsiIdentity;

use super::final_image_validation::{validate_terminal_image, validate_terminal_native_fuel_image};
use super::{
    TerminalObjectArtifact, TerminalObjectBoundarySettlement, TerminalObjectFuelAttribution,
    TerminalObjectFunction, TerminalObjectPortEffect, ValidatedTerminalNativeFuelArtifact,
    ValidatedTerminalNativeFuelFunction,
};
use omega_terminal_installation_evidence::NativeFuelTargetPlanProjection;

pub fn emit_terminal_object_container(
    artifact: &TerminalObjectArtifact,
) -> TerminalObjectContainer {
    TerminalObjectContainer {
        terminal_psi: artifact.terminal_psi,
        output: emit_omega_object_container(ObjectContainerInput {
            target: artifact.target,
            object: &artifact.object,
            relocations: &artifact.relocations,
            text_bytes: &artifact.text_bytes,
            data_bytes: &[],
        }),
    }
}

pub fn emit_terminal_native_fuel_object_container(
    artifact: &ValidatedTerminalNativeFuelArtifact,
) -> TerminalObjectContainer {
    TerminalObjectContainer {
        terminal_psi: artifact.semantic_artifact().terminal_psi(),
        output: emit_omega_object_container(ObjectContainerInput {
            target: artifact.semantic_artifact().target(),
            object: artifact.object(),
            relocations: artifact.relocations(),
            text_bytes: artifact.text_bytes(),
            data_bytes: &[],
        }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalObjectContainer {
    pub terminal_psi: TerminalPsiIdentity,
    pub output: ObjectContainerOutput,
}

pub fn can_emit_terminal_executable_image(target: NativeTarget) -> bool {
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
/// The clean lane admits only typed internal-call relocations. Final-text
/// mutation outside their architecture-specific immediate bits, imports,
/// appended thunks, overlapping/missing function spans, and unclassified
/// executable bytes are hard failures.
pub fn emit_terminal_executable_image(
    artifact: &TerminalObjectArtifact,
    subsystem: u16,
) -> Result<TerminalExecutableImage, Diagnostic> {
    if !can_emit_terminal_executable_image(artifact.target) {
        return Err(Diagnostic::error(format!(
            "cannot emit terminal-Psi executable image for {:?}",
            artifact.target
        )));
    }
    let image = omega_image::build_final_image(FinalImageInput {
        target: artifact.target,
        object: &artifact.object,
        relocations: &artifact.relocations,
        text_bytes: &artifact.text_bytes,
        data_bytes: &[],
    });
    let output = match (artifact.target.object_format, artifact.target.architecture) {
        (ObjectFormat::Elf, Architecture::Aarch64) => {
            omega_image_elf::emit_elf_aarch64_executable(image)
        }
        (ObjectFormat::Elf, Architecture::X86_64) => {
            omega_image_elf::emit_elf_x86_64_executable(image)
        }
        (ObjectFormat::MachO, Architecture::Aarch64) => {
            omega_image_macho::emit_macho_aarch64_executable(image)
        }
        (ObjectFormat::Coff, Architecture::X86_64) => {
            omega_image_pe::emit_pe_x86_64_executable(image, subsystem)
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "cannot emit terminal-Psi executable image for {:?}",
                artifact.target
            )));
        }
    }?;
    let mut output = emitted_direct_executable_output(output);
    output.compiler_text_validation = Some(validate_terminal_image(artifact, &output)?);
    Ok(TerminalExecutableImage {
        terminal_psi: artifact.terminal_psi,
        target: artifact.target,
        subsystem: matches!(artifact.target.object_format, ObjectFormat::Coff).then_some(subsystem),
        functions: artifact.functions.clone(),
        fuel_attribution: artifact.fuel_attribution.clone(),
        port_effects: artifact.port_effects.clone(),
        boundary_settlements: artifact.boundary_settlements.clone(),
        output,
    })
}

pub fn emit_terminal_native_fuel_executable_image(
    artifact: &ValidatedTerminalNativeFuelArtifact,
    subsystem: u16,
) -> Result<TerminalNativeFuelExecutableImage, Diagnostic> {
    let target = artifact.semantic_artifact().target();
    if !can_emit_terminal_executable_image(target) {
        return Err(Diagnostic::error(format!(
            "cannot emit metered terminal-Psi executable image for {target:?}"
        )));
    }
    let image = omega_image::build_final_image(FinalImageInput {
        target,
        object: artifact.object(),
        relocations: artifact.relocations(),
        text_bytes: artifact.text_bytes(),
        data_bytes: &[],
    });
    let output = match (target.object_format, target.architecture) {
        (ObjectFormat::Elf, Architecture::Aarch64) => {
            omega_image_elf::emit_elf_aarch64_executable(image)
        }
        (ObjectFormat::Elf, Architecture::X86_64) => {
            omega_image_elf::emit_elf_x86_64_executable(image)
        }
        (ObjectFormat::MachO, Architecture::Aarch64) => {
            omega_image_macho::emit_macho_aarch64_executable(image)
        }
        (ObjectFormat::Coff, Architecture::X86_64) => {
            omega_image_pe::emit_pe_x86_64_executable(image, subsystem)
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "cannot emit metered terminal-Psi executable image for {target:?}"
            )));
        }
    }?;
    let mut output = emitted_direct_executable_output(output);
    output.compiler_text_validation = Some(validate_terminal_native_fuel_image(artifact, &output)?);
    Ok(TerminalNativeFuelExecutableImage {
        terminal_psi: artifact.semantic_artifact().terminal_psi(),
        target,
        subsystem: matches!(target.object_format, ObjectFormat::Coff).then_some(subsystem),
        target_policy: artifact.target_policy(),
        functions: artifact.functions().to_vec(),
        output,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeFuelExecutableImage {
    terminal_psi: TerminalPsiIdentity,
    target: NativeTarget,
    subsystem: Option<u16>,
    target_policy: NativeFuelTargetPlanProjection,
    functions: Vec<ValidatedTerminalNativeFuelFunction>,
    output: EmittedImageOutput,
}

impl TerminalNativeFuelExecutableImage {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn subsystem(&self) -> Option<u16> {
        self.subsystem
    }

    pub const fn target_policy(&self) -> NativeFuelTargetPlanProjection {
        self.target_policy
    }

    pub fn functions(&self) -> &[ValidatedTerminalNativeFuelFunction] {
        &self.functions
    }

    pub const fn output(&self) -> &EmittedImageOutput {
        &self.output
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalExecutableImage {
    terminal_psi: TerminalPsiIdentity,
    target: NativeTarget,
    subsystem: Option<u16>,
    functions: Vec<TerminalObjectFunction>,
    fuel_attribution: Vec<TerminalObjectFuelAttribution>,
    port_effects: Vec<TerminalObjectPortEffect>,
    boundary_settlements: Vec<TerminalObjectBoundarySettlement>,
    output: EmittedImageOutput,
}

impl TerminalExecutableImage {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    /// PE/COFF subsystem selected by the writer. Other formats carry no
    /// subsystem fact because the argument is not interpreted by their writer.
    pub const fn subsystem(&self) -> Option<u16> {
        self.subsystem
    }

    pub const fn output(&self) -> &EmittedImageOutput {
        &self.output
    }

    pub fn boundary_settlements(&self) -> &[TerminalObjectBoundarySettlement] {
        &self.boundary_settlements
    }

    pub fn functions(&self) -> &[TerminalObjectFunction] {
        &self.functions
    }

    pub fn port_effects(&self) -> &[TerminalObjectPortEffect] {
        &self.port_effects
    }

    pub fn fuel_attribution(&self) -> &[TerminalObjectFuelAttribution] {
        &self.fuel_attribution
    }
}

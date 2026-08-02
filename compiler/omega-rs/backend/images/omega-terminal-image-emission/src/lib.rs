#![forbid(unsafe_code)]

//! Standalone object and executable-image emission for the clean terminal-Psi
//! realization lane.
//!
//! This crate consumes only owned terminal machine-code functions. It does not
//! reconstruct the legacy `EncodedMachineCode` carrier or any source-shaped
//! lowering state. Current terminal functions contain no calls, data, imports,
//! or relocations, so final compiler text must be byte-for-byte identical to
//! the emitted input and every byte must belong to one provenance-bearing
//! function region.

use omega_core::diagnostics::Diagnostic;
use omega_image::{
    CompilerTextValidationEvidence, EmittedImageOutput, FinalExecutableRegionOrigin,
    FinalImageInput, emitted_direct_executable_output,
};
use omega_object_file::{
    ObjectContainerInput, ObjectContainerOutput, ObjectPlan, ObjectSymbolHandle, RelocationPlan,
    SectionKind, SectionPlan, SymbolKind, SymbolPlan, SymbolSection, emit_omega_object_container,
    entry_symbol_name,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_machine_code::TerminalMachineCodePlan;
use omega_terminal_target_operations::TerminalPsiProvenance;
use psi_core::MachineId;
use psi_terminal::TerminalPsiIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalObjectArtifact {
    terminal_psi: TerminalPsiIdentity,
    target: NativeTarget,
    entry: MachineId,
    object: ObjectPlan,
    relocations: RelocationPlan,
    text_bytes: Vec<u8>,
    functions: Vec<TerminalObjectFunction>,
}

impl TerminalObjectArtifact {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub const fn object(&self) -> &ObjectPlan {
        &self.object
    }

    pub const fn relocations(&self) -> &RelocationPlan {
        &self.relocations
    }

    pub fn text_bytes(&self) -> &[u8] {
        &self.text_bytes
    }

    pub fn functions(&self) -> &[TerminalObjectFunction] {
        &self.functions
    }

    pub fn entry_function(&self) -> &TerminalObjectFunction {
        self.functions
            .iter()
            .find(|function| function.machine == self.entry)
            .expect("artifact construction requires one entry function")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalObjectFunction {
    pub machine: MachineId,
    pub provenance: TerminalPsiProvenance,
    pub symbol: ObjectSymbolHandle,
    pub text_offset: usize,
    pub byte_count: usize,
}

impl TerminalObjectFunction {
    pub fn bytes<'artifact>(&self, artifact: &'artifact TerminalObjectArtifact) -> &'artifact [u8] {
        &artifact.text_bytes[self.text_offset..self.text_offset + self.byte_count]
    }
}

/// Construct a self-contained object plan and exact text carrier.
///
/// Function order is semantic-artifact order and must already be canonical by
/// `MachineId`; this boundary rejects alternate ordering rather than silently
/// normalizing it. Each function gets exactly one symbol and one retained Psi
/// provenance row.
pub fn build_terminal_object_artifact(
    plan: &TerminalMachineCodePlan,
) -> Result<TerminalObjectArtifact, TerminalObjectError> {
    if plan.functions.is_empty() {
        return Err(TerminalObjectError::EmptyPlan);
    }
    let mut previous = None;
    let mut saw_entry = false;
    let mut text_size = 0usize;
    for function in &plan.functions {
        if let Some(previous) = previous
            && previous >= function.machine
        {
            return Err(TerminalObjectError::NonCanonicalFunctionOrder {
                previous,
                current: function.machine,
            });
        }
        if function.bytes.is_empty() {
            return Err(TerminalObjectError::EmptyFunction(function.machine));
        }
        previous = Some(function.machine);
        saw_entry |= function.machine == plan.entry;
        text_size = text_size
            .checked_add(function.bytes.len())
            .ok_or(TerminalObjectError::TextSizeOverflow)?;
    }
    if !saw_entry {
        return Err(TerminalObjectError::EntryFunctionMissing(plan.entry));
    }

    let mut object = ObjectPlan::with_capacity(plan.target, 1, plan.functions.len());
    object.layout.sections.insert(SectionPlan {
        kind: SectionKind::Text,
        size: text_size,
        alignment: 16,
    });

    let mut text_bytes = Vec::with_capacity(text_size);
    let mut functions = Vec::with_capacity(plan.functions.len());
    for function in &plan.functions {
        let text_offset = text_bytes.len();
        text_bytes.extend_from_slice(&function.bytes);
        let is_entry = function.machine == plan.entry;
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: if is_entry {
                entry_symbol_name(plan.target)
            } else {
                format!("omega_terminal_machine_{}", function.machine.get())
            },
            section: SymbolSection::Section(SectionKind::Text),
            offset: text_offset,
            size: function.bytes.len(),
            kind: SymbolKind::Function,
            import_library: String::new(),
        });
        if is_entry {
            object.layout.entry_symbol = symbol;
        }
        functions.push(TerminalObjectFunction {
            machine: function.machine,
            provenance: function.provenance.clone(),
            symbol,
            text_offset,
            byte_count: function.bytes.len(),
        });
    }

    Ok(TerminalObjectArtifact {
        terminal_psi: plan.terminal_psi,
        target: plan.target,
        entry: plan.entry,
        object,
        relocations: RelocationPlan::with_target(plan.target),
        text_bytes,
        functions,
    })
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalObjectContainer {
    pub terminal_psi: TerminalPsiIdentity,
    pub output: ObjectContainerOutput,
}

pub fn can_emit_terminal_executable_image(target: NativeTarget) -> bool {
    matches!(
        (target.object_format, target.architecture),
        (ObjectFormat::Elf, Architecture::Aarch64)
            | (ObjectFormat::Elf, Architecture::X86_64)
            | (ObjectFormat::MachO, Architecture::Aarch64)
            | (ObjectFormat::Coff, Architecture::X86_64)
    )
}

/// Emit and validate one direct executable image.
///
/// The current clean lane has no relocation or import vocabulary. Therefore
/// any final-text mutation, appended thunk, overlapping/missing function span,
/// or unclassified executable byte is a hard failure.
pub fn emit_terminal_executable_image(
    artifact: &TerminalObjectArtifact,
    subsystem: u16,
) -> Result<TerminalExecutableImage, Diagnostic> {
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
    validate_terminal_image(artifact, &output)?;
    output.compiler_text_validation = Some(exact_text_evidence(&artifact.text_bytes));
    Ok(TerminalExecutableImage {
        terminal_psi: artifact.terminal_psi,
        output,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalExecutableImage {
    pub terminal_psi: TerminalPsiIdentity,
    pub output: EmittedImageOutput,
}

fn validate_terminal_image(
    artifact: &TerminalObjectArtifact,
    output: &EmittedImageOutput,
) -> Result<(), Diagnostic> {
    if output.final_text_bytes != artifact.text_bytes {
        return Err(Diagnostic::error(
            "terminal-Psi final .text differs from its relocation-free machine-code artifact",
        ));
    }
    if let Some(gap) = output.executable_regions.unclassified_gaps.first() {
        return Err(Diagnostic::error(format!(
            "terminal-Psi executable inventory left {} unclassified byte(s) at .text offset {}",
            gap.byte_count, gap.section_offset
        )));
    }
    let compiler_regions = output
        .executable_regions
        .regions
        .iter()
        .filter(|region| region.origin == FinalExecutableRegionOrigin::CompilerFunction)
        .collect::<Vec<_>>();
    if compiler_regions.len() != artifact.functions.len() {
        return Err(Diagnostic::error(format!(
            "terminal-Psi image retained {} compiler function region(s), expected {}",
            compiler_regions.len(),
            artifact.functions.len()
        )));
    }
    for function in &artifact.functions {
        let symbol = omega_object_file::object_symbol_name(&artifact.object, function.symbol);
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
    Ok(())
}

fn exact_text_evidence(bytes: &[u8]) -> CompilerTextValidationEvidence {
    let text_fingerprint = fingerprint_bytes(bytes);
    let relocation_envelope_fingerprint = FNV_OFFSET;
    let mut relocation_derivation = FNV_OFFSET;
    fingerprint_into(&mut relocation_derivation, &text_fingerprint.to_le_bytes());
    fingerprint_into(&mut relocation_derivation, &text_fingerprint.to_le_bytes());
    fingerprint_into(
        &mut relocation_derivation,
        &relocation_envelope_fingerprint.to_le_bytes(),
    );
    fingerprint_into(&mut relocation_derivation, &0_u64.to_le_bytes());
    let checked_instruction_fingerprint = FNV_OFFSET;
    let mut derivation_fingerprint = FNV_OFFSET;
    fingerprint_into(
        &mut derivation_fingerprint,
        &relocation_derivation.to_le_bytes(),
    );
    fingerprint_into(
        &mut derivation_fingerprint,
        &checked_instruction_fingerprint.to_le_bytes(),
    );
    fingerprint_into(&mut derivation_fingerprint, &0_u64.to_le_bytes());
    CompilerTextValidationEvidence {
        encoded_text_fingerprint: text_fingerprint,
        final_compiler_text_fingerprint: text_fingerprint,
        relocation_envelope_fingerprint,
        checked_instruction_validation_fingerprint: checked_instruction_fingerprint,
        derivation_fingerprint,
        text_relocation_count: 0,
        checked_instruction_validation_count: 0,
    }
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

fn fingerprint_bytes(bytes: &[u8]) -> u64 {
    let mut fingerprint = FNV_OFFSET;
    fingerprint_into(&mut fingerprint, bytes);
    fingerprint
}

fn fingerprint_into(fingerprint: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *fingerprint ^= u64::from(*byte);
        *fingerprint = fingerprint.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalObjectError {
    EmptyPlan,
    NonCanonicalFunctionOrder {
        previous: MachineId,
        current: MachineId,
    },
    EmptyFunction(MachineId),
    EntryFunctionMissing(MachineId),
    TextSizeOverflow,
}

impl std::fmt::Display for TerminalObjectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalObjectError {}

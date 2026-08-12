#![forbid(unsafe_code)]

//! Standalone object and executable-image emission for the clean terminal-Psi
//! realization lane.
//!
//! This crate consumes only owned terminal machine-code functions. It does not
//! reconstruct the legacy `EncodedMachineCode` carrier or any source-shaped
//! lowering state. Typed internal-call relocations may change only their exact
//! architecture-native immediate fields; every other compiler-authored bit and
//! every provenance-bearing function region remains final-byte validated.
//!
//! The adjacent canonical installation record is manifest metadata over the
//! resulting sealed image. It does not grant executable authority or replace
//! the separate native admission, placement, and retirement ladder.

mod installation;

pub use installation::*;

use omega_image::{
    CompilerTextValidationEvidence, EmittedImageOutput, FinalExecutableRegionOrigin,
    FinalImageInput, emitted_direct_executable_output, validate_final_text_relocation_envelope,
};
use omega_object_file::{
    ObjectContainerInput, ObjectContainerOutput, ObjectPlan, ObjectSymbolHandle, RelocationKind,
    RelocationOrigin, RelocationPlan, RelocationRecord, SectionKind, SectionPlan, SymbolKind,
    SymbolPlan, SymbolSection, emit_omega_object_container, entry_symbol_name,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_machine_code::{
    TerminalBoundarySettlementRecord, TerminalMachineCodePlan, TerminalPortEffectRecord,
};
use omega_terminal_target_operations::TerminalPsiProvenance;
use psi_core::MachineId;
use psi_diagnostics::Diagnostic;
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
    port_effects: Vec<TerminalObjectPortEffect>,
    boundary_settlements: Vec<TerminalObjectBoundarySettlement>,
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

    pub fn boundary_settlements(&self) -> &[TerminalObjectBoundarySettlement] {
        &self.boundary_settlements
    }

    pub fn port_effects(&self) -> &[TerminalObjectPortEffect] {
        &self.port_effects
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalObjectBoundarySettlement {
    pub machine: MachineId,
    pub settlement: TerminalBoundarySettlementRecord,
    /// Absolute offset in the object `.text` section.
    pub text_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalObjectPortEffect {
    pub machine: MachineId,
    pub effect: TerminalPortEffectRecord,
    /// Absolute offset in the object `.text` section.
    pub text_offset: usize,
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
        if function
            .internal_calls
            .windows(2)
            .any(|pair| pair[0].offset >= pair[1].offset)
        {
            return Err(TerminalObjectError::NonCanonicalInternalCallOrder(
                function.machine,
            ));
        }
        let mut call_operations = std::collections::BTreeSet::new();
        for call in &function.internal_calls {
            if !function.provenance.operations.contains(&call.psi_operation) {
                return Err(TerminalObjectError::InternalCallOperationNotInProvenance {
                    caller: function.machine,
                    operation: call.psi_operation,
                });
            }
            if !call_operations.insert(call.psi_operation) {
                return Err(TerminalObjectError::DuplicateInternalCallOperation {
                    caller: function.machine,
                    operation: call.psi_operation,
                });
            }
        }
        if function.port_effects.windows(2).any(|pair| {
            (pair[0].code_offset, pair[0].operation_ordinal)
                >= (pair[1].code_offset, pair[1].operation_ordinal)
        }) {
            return Err(TerminalObjectError::NonCanonicalPortEffectOrder(
                function.machine,
            ));
        }
        let mut port_operations = std::collections::BTreeSet::new();
        for effect in &function.port_effects {
            let end = effect.code_offset.checked_add(effect.byte_count).ok_or(
                TerminalObjectError::PortEffectOutsideFunction {
                    machine: function.machine,
                    operation: effect.psi_operation,
                },
            )?;
            if end > function.bytes.len() || effect.byte_count == 0 {
                return Err(TerminalObjectError::PortEffectOutsideFunction {
                    machine: function.machine,
                    operation: effect.psi_operation,
                });
            }
            if !function
                .provenance
                .operations
                .contains(&effect.psi_operation)
            {
                return Err(TerminalObjectError::PortEffectOperationNotInProvenance {
                    machine: function.machine,
                    operation: effect.psi_operation,
                });
            }
            if !port_operations.insert(effect.psi_operation) {
                return Err(TerminalObjectError::DuplicatePortEffectOperation {
                    machine: function.machine,
                    operation: effect.psi_operation,
                });
            }
            if plan.target.architecture != Architecture::X86_64
                || function.bytes[effect.code_offset..end]
                    != omega_x86_encoding::encode_immediate_port_write(effect.port, effect.value)
            {
                return Err(TerminalObjectError::PortEffectBytesMismatch {
                    machine: function.machine,
                    operation: effect.psi_operation,
                });
            }
        }
        if function.boundary_settlements.windows(2).any(|pair| {
            (pair[0].code_offset, pair[0].operation_ordinal)
                >= (pair[1].code_offset, pair[1].operation_ordinal)
        }) {
            return Err(TerminalObjectError::NonCanonicalBoundarySettlementOrder(
                function.machine,
            ));
        }
        let mut settlement_operations = std::collections::BTreeSet::new();
        for settlement in &function.boundary_settlements {
            if settlement.code_offset > function.bytes.len() {
                return Err(TerminalObjectError::BoundarySettlementOutsideFunction {
                    machine: function.machine,
                    operation: settlement.psi_operation,
                });
            }
            if !function
                .provenance
                .operations
                .contains(&settlement.psi_operation)
            {
                return Err(
                    TerminalObjectError::BoundarySettlementOperationNotInProvenance {
                        machine: function.machine,
                        operation: settlement.psi_operation,
                    },
                );
            }
            if !settlement_operations.insert(settlement.psi_operation) {
                return Err(TerminalObjectError::DuplicateBoundarySettlementOperation {
                    machine: function.machine,
                    operation: settlement.psi_operation,
                });
            }
            let realization = settlement.realization;
            if function
                .port_effects
                .iter()
                .filter(|effect| {
                    effect.psi_operation == realization.effect_operation
                        && effect.service == realization.service
                        && effect.port == realization.port
                        && effect.value == realization.value
                        && effect.operation_ordinal.checked_add(1)
                            == Some(settlement.operation_ordinal)
                        && effect.code_offset.checked_add(effect.byte_count)
                            == Some(settlement.code_offset)
                })
                .count()
                != 1
            {
                return Err(TerminalObjectError::BoundaryRealizationMismatch {
                    machine: function.machine,
                    operation: settlement.psi_operation,
                });
            }
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
    let mut port_effects = Vec::new();
    let mut boundary_settlements = Vec::new();
    let mut symbols_by_machine = std::collections::BTreeMap::new();
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
        symbols_by_machine.insert(function.machine, symbol);
        for effect in &function.port_effects {
            port_effects.push(TerminalObjectPortEffect {
                machine: function.machine,
                effect: effect.clone(),
                text_offset: text_offset
                    .checked_add(effect.code_offset)
                    .ok_or(TerminalObjectError::TextSizeOverflow)?,
            });
        }
        for settlement in &function.boundary_settlements {
            boundary_settlements.push(TerminalObjectBoundarySettlement {
                machine: function.machine,
                settlement: settlement.clone(),
                text_offset: text_offset
                    .checked_add(settlement.code_offset)
                    .ok_or(TerminalObjectError::TextSizeOverflow)?,
            });
        }
        functions.push(TerminalObjectFunction {
            machine: function.machine,
            provenance: function.provenance.clone(),
            symbol,
            text_offset,
            byte_count: function.bytes.len(),
        });
    }

    let mut relocations = RelocationPlan::with_record_capacity(
        plan.target,
        plan.functions
            .iter()
            .map(|function| function.internal_calls.len())
            .sum(),
    );
    for (function, emitted) in plan.functions.iter().zip(&functions) {
        for call in &function.internal_calls {
            let target_symbol = symbols_by_machine.get(&call.target).copied().ok_or(
                TerminalObjectError::UnknownInternalCallTarget {
                    caller: function.machine,
                    target: call.target,
                },
            )?;
            let (kind, byte_width) = validate_internal_call_site(
                plan.target.architecture,
                function.machine,
                &function.bytes,
                *call,
            )?;
            let offset = emitted
                .text_offset
                .checked_add(call.offset)
                .ok_or(TerminalObjectError::TextSizeOverflow)?;
            relocations.push_record(RelocationRecord {
                origin: RelocationOrigin::SemanticOperation {
                    function_symbol_handle: emitted.symbol,
                    operation_identity: call.psi_operation.get(),
                },
                section: SectionKind::Text,
                offset,
                byte_width,
                symbol_handle: target_symbol,
                addend: 0,
                kind,
            });
        }
    }

    Ok(TerminalObjectArtifact {
        terminal_psi: plan.terminal_psi,
        target: plan.target,
        entry: plan.entry,
        object,
        relocations,
        text_bytes,
        functions,
        port_effects,
        boundary_settlements,
    })
}

fn validate_internal_call_site(
    architecture: Architecture,
    caller: MachineId,
    bytes: &[u8],
    call: omega_terminal_machine_code::TerminalInternalCallRelocation,
) -> Result<(RelocationKind, usize), TerminalObjectError> {
    let valid = match architecture {
        Architecture::X86_64 => {
            call.offset >= 1
                && bytes.get(call.offset - 1) == Some(&0xe8)
                && bytes.get(call.offset..call.offset.saturating_add(4)) == Some(&[0; 4])
        }
        Architecture::Aarch64 => {
            call.offset.is_multiple_of(4)
                && bytes.get(call.offset..call.offset.saturating_add(4))
                    == Some(&0x9400_0000u32.to_le_bytes())
        }
    };
    if !valid {
        return Err(TerminalObjectError::InvalidInternalCallSite {
            caller,
            operation: call.psi_operation,
            offset: call.offset,
        });
    }
    Ok(match architecture {
        Architecture::X86_64 => (RelocationKind::X86_64Relative32, 4),
        Architecture::Aarch64 => (RelocationKind::Aarch64Branch26, 4),
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
        port_effects: artifact.port_effects.clone(),
        boundary_settlements: artifact.boundary_settlements.clone(),
        output,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalExecutableImage {
    terminal_psi: TerminalPsiIdentity,
    target: NativeTarget,
    subsystem: Option<u16>,
    functions: Vec<TerminalObjectFunction>,
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
}

fn validate_terminal_image(
    artifact: &TerminalObjectArtifact,
    output: &EmittedImageOutput,
) -> Result<CompilerTextValidationEvidence, Diagnostic> {
    if output.final_image_imports != 0 {
        return Err(Diagnostic::error(
            "terminal-Psi internal-call image unexpectedly retained imports",
        ));
    }
    if output.final_image_relocations != artifact.relocations.record_count() {
        return Err(Diagnostic::error(format!(
            "terminal-Psi image retained {} relocation(s), expected {}",
            output.final_image_relocations,
            artifact.relocations.record_count()
        )));
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
    validate_final_text_relocation_envelope(
        &artifact.text_bytes,
        &output.final_text_bytes,
        &artifact.relocations,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalObjectError {
    EmptyPlan,
    NonCanonicalFunctionOrder {
        previous: MachineId,
        current: MachineId,
    },
    EmptyFunction(MachineId),
    NonCanonicalInternalCallOrder(MachineId),
    NonCanonicalPortEffectOrder(MachineId),
    NonCanonicalBoundarySettlementOrder(MachineId),
    UnknownInternalCallTarget {
        caller: MachineId,
        target: MachineId,
    },
    InvalidInternalCallSite {
        caller: MachineId,
        operation: psi_core::OperationId,
        offset: usize,
    },
    InternalCallOperationNotInProvenance {
        caller: MachineId,
        operation: psi_core::OperationId,
    },
    DuplicateInternalCallOperation {
        caller: MachineId,
        operation: psi_core::OperationId,
    },
    PortEffectOutsideFunction {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    PortEffectOperationNotInProvenance {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    DuplicatePortEffectOperation {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    PortEffectBytesMismatch {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    BoundarySettlementOutsideFunction {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    BoundarySettlementOperationNotInProvenance {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    DuplicateBoundarySettlementOperation {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    BoundaryRealizationMismatch {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    EntryFunctionMissing(MachineId),
    TextSizeOverflow,
}

impl std::fmt::Display for TerminalObjectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalObjectError {}

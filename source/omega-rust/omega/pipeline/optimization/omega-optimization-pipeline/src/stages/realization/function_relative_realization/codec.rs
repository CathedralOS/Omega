use super::prelude::*;
use super::{
    FunctionRelativeOptimizationRealizationScope, FunctionRelativeOptimizationRealizationStage,
    FunctionRelativeOptimizationUnavailableData,
    model::{
        FunctionRelativeOptimizationRealizationManifest,
        FunctionRelativeOptimizationRealizationStatistics,
    },
};

const MANIFEST_MAGIC: &[u8; 8] = b"OMGFRM\0\0";
const MANIFEST_VERSION: u32 = 8;

impl FunctionRelativeOptimizationRealizationManifest {
    pub fn recomputed_identity(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        let mut canonical = Vec::new();
        canonical
            .extend_from_slice(b"omega.function-relative-optimization-realization-manifest.v8\0");
        canonical.extend_from_slice(&encode_manifest_content(self));
        FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(&canonical)
    }

    pub fn encode(&self) -> Vec<u8> {
        let content = encode_manifest_content(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(MANIFEST_MAGIC);
        encoded.extend_from_slice(&MANIFEST_VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(
        encoded: &[u8],
    ) -> Result<Self, FunctionRelativeOptimizationRealizationManifestDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(MANIFEST_MAGIC.len())? != MANIFEST_MAGIC {
            return Err(FunctionRelativeOptimizationRealizationManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != MANIFEST_VERSION {
            return Err(
                FunctionRelativeOptimizationRealizationManifestDecodeError::UnsupportedVersion(
                    version,
                ),
            );
        }
        let identity =
            FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes(cursor.array()?);
        let stage = match cursor.byte()? {
            1 => FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1,
            tag => {
                return Err(
                    FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownStage(tag),
                );
            }
        };
        let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let selected_lowering_selections =
            OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let selected_lowering_completion = match cursor.byte()? {
            0 => None,
            1 => Some(SelectedLoweringOptimizationCompletionIdentity::from_bytes(
                cursor.array()?,
            )),
            tag => {
                return Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownSelectedLoweringCompletionStatus(tag));
            }
        };
        let allocation_recovery_selections =
            OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let post_allocation_machine_selections =
            OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let function_relative_layout_selections =
            OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let pre_physical_manifest =
            PrePhysicalOptimizationManifestIdentity::from_bytes(cursor.array()?);
        let post_allocation_manifest =
            PostAllocationOptimizationManifestIdentity::from_bytes(cursor.array()?);
        let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let pre_allocation_machine_effects =
            omega_machine_optimizer::PreAllocationMachineEffectIdentity::from_bytes(
                cursor.array()?,
            );
        let post_allocation_machine =
            omega_machine_optimizer::PostAllocationMachineIdentity::from_bytes(cursor.array()?);
        let baseline_pre_layout = SelectedFormEncodingIdentity::from_bytes(cursor.array()?);
        let pre_layout = SelectedFormEncodingIdentity::from_bytes(cursor.array()?);
        let baseline_resolved_layout =
            ResolvedSelectedFormLayoutIdentity::from_bytes(cursor.array()?);
        let resolved_layout = ResolvedSelectedFormLayoutIdentity::from_bytes(cursor.array()?);
        let x86_branch_relaxation = match cursor.byte()? {
            0 => None,
            1 => Some(X86BranchRelaxationIdentity::from_bytes(cursor.array()?)),
            tag => {
                return Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownX86BranchRelaxationStatus(tag));
            }
        };
        let aarch64_cbnz_fusion = match cursor.byte()? {
            0 => None,
            1 => Some(
                omega_machine_optimizer::Aarch64CbnzFusionIdentity::from_bytes(cursor.array()?),
            ),
            tag => {
                return Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownAarch64CbnzFusionStatus(tag));
            }
        };
        let aarch64_movn_materialization = match cursor.byte()? {
            0 => None,
            1 => Some(
                omega_machine_optimizer::Aarch64MovnMaterializationIdentity::from_bytes(
                    cursor.array()?,
                ),
            ),
            tag => {
                return Err(
                    FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownAarch64MovnMaterializationStatus(tag),
                );
            }
        };
        let whole_function_exit_contract =
            WholeFunctionExitContractIdentity::from_bytes(cursor.array()?);
        let target = decode_target(&mut cursor)?;
        let layout_policy = match cursor.byte()? {
            1 => SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1,
            2 => SelectedFunctionLayoutPolicy::SingleEntryBlockV1,
            3 => SelectedFunctionLayoutPolicy::StructuralUnitCallThenReturnSingleEntryBlockV1,
            tag => {
                return Err(
                    FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownLayoutPolicy(
                        tag,
                    ),
                );
            }
        };
        let scope = match cursor.byte()? {
            1 => FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
            tag => {
                return Err(
                    FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownScope(tag),
                );
            }
        };
        let statistics = FunctionRelativeOptimizationRealizationStatistics {
            functions: u64::from_le_bytes(cursor.array()?),
            blocks: u64::from_le_bytes(cursor.array()?),
            instructions: u64::from_le_bytes(cursor.array()?),
            bytes: u64::from_le_bytes(cursor.array()?),
            resolved_conditional_branches: u64::from_le_bytes(cursor.array()?),
            structural_unit_functions: u64::from_le_bytes(cursor.array()?),
            structural_unit_blocks: u64::from_le_bytes(cursor.array()?),
            structural_unit_instructions: u64::from_le_bytes(cursor.array()?),
            structural_unit_bytes: u64::from_le_bytes(cursor.array()?),
            unresolved_internal_machine_fixups: u64::from_le_bytes(cursor.array()?),
        };
        let unavailable = [
            decode_unavailable(&mut cursor)?,
            decode_unavailable(&mut cursor)?,
            decode_unavailable(&mut cursor)?,
            decode_unavailable(&mut cursor)?,
            decode_unavailable(&mut cursor)?,
            decode_unavailable(&mut cursor)?,
            decode_unavailable(&mut cursor)?,
            decode_unavailable(&mut cursor)?,
        ];
        if cursor.remaining() != 0 {
            return Err(FunctionRelativeOptimizationRealizationManifestDecodeError::TrailingBytes);
        }
        let manifest = Self {
            identity,
            stage,
            selections,
            selected_lowering_selections,
            selected_lowering_completion,
            allocation_recovery_selections,
            post_allocation_machine_selections,
            function_relative_layout_selections,
            pre_physical_manifest,
            post_allocation_manifest,
            selected,
            pre_allocation_machine_effects,
            post_allocation_machine,
            baseline_pre_layout,
            pre_layout,
            baseline_resolved_layout,
            resolved_layout,
            x86_branch_relaxation,
            aarch64_cbnz_fusion,
            aarch64_movn_materialization,
            whole_function_exit_contract,
            target,
            layout_policy,
            scope,
            statistics,
            frame: unavailable[0],
            machine_emission: unavailable[1],
            section_placement: unavailable[2],
            symbols: unavailable[3],
            object_relocations: unavailable[4],
            executable_image: unavailable[5],
            installation: unavailable[6],
            publication: unavailable[7],
        };
        if usize::from(manifest.x86_branch_relaxation.is_some())
            + usize::from(manifest.aarch64_cbnz_fusion.is_some())
            + usize::from(manifest.aarch64_movn_materialization.is_some())
            > 1
        {
            return Err(
                FunctionRelativeOptimizationRealizationManifestDecodeError::ConflictingPhysicalTransformations,
            );
        }
        if manifest.identity != manifest.recomputed_identity() {
            return Err(
                FunctionRelativeOptimizationRealizationManifestDecodeError::IdentityMismatch,
            );
        }
        Ok(manifest)
    }

    pub fn render_text(&self) -> String {
        let mut output = String::new();
        writeln!(output, "Omega function-relative optimization realization").unwrap();
        writeln!(
            output,
            "stage: validated function-relative selected forms and whole-function exit v1"
        )
        .unwrap();
        writeln!(output, "manifest identity: {}", hex(&self.identity.bytes())).unwrap();
        writeln!(
            output,
            "full named suite: {}",
            hex(&self.selections.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "selected-lowering suite: {}",
            hex(&self.selected_lowering_selections.bytes())
        )
        .unwrap();
        match self.selected_lowering_completion {
            Some(identity) => writeln!(
                output,
                "selected-lowering completion: {}",
                hex(&identity.bytes())
            )
            .unwrap(),
            None => writeln!(output, "selected-lowering completion: not run").unwrap(),
        }
        writeln!(
            output,
            "allocation-recovery suite: {}",
            hex(&self.allocation_recovery_selections.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "post-allocation-machine suite: {}",
            hex(&self.post_allocation_machine_selections.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "function-relative-layout suite: {}",
            hex(&self.function_relative_layout_selections.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "pre-physical manifest: {}",
            hex(&self.pre_physical_manifest.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "post-allocation manifest: {}",
            hex(&self.post_allocation_manifest.bytes())
        )
        .unwrap();
        writeln!(output, "selected CFG: {}", hex(&self.selected.bytes())).unwrap();
        writeln!(
            output,
            "pre-allocation machine effects: {}",
            hex(&self.pre_allocation_machine_effects.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "post-allocation machine: {}",
            hex(&self.post_allocation_machine.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "baseline pre-layout encoding: {}",
            hex(&self.baseline_pre_layout.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "pre-layout encoding: {}",
            hex(&self.pre_layout.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "baseline resolved layout: {}",
            hex(&self.baseline_resolved_layout.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "final resolved layout: {}",
            hex(&self.resolved_layout.bytes())
        )
        .unwrap();
        match self.x86_branch_relaxation {
            Some(identity) => {
                writeln!(output, "x86 branch relaxation: {}", hex(&identity.bytes())).unwrap()
            }
            None => writeln!(output, "x86 branch relaxation: not run").unwrap(),
        }
        match self.aarch64_cbnz_fusion {
            Some(identity) => {
                writeln!(output, "AArch64 CBNZ fusion: {}", hex(&identity.bytes())).unwrap()
            }
            None => writeln!(output, "AArch64 CBNZ fusion: not run").unwrap(),
        }
        match self.aarch64_movn_materialization {
            Some(identity) => writeln!(
                output,
                "AArch64 MOVN materialization: {}",
                hex(&identity.bytes())
            )
            .unwrap(),
            None => writeln!(output, "AArch64 MOVN materialization: not run").unwrap(),
        }
        writeln!(
            output,
            "whole-function exit contract: {}",
            hex(&self.whole_function_exit_contract.bytes())
        )
        .unwrap();
        writeln!(
            output,
            "target: {}/{} pointers={}/{}",
            architecture_name(self.target.architecture),
            object_format_name(self.target.object_format),
            self.target.pointer_size,
            self.target.pointer_alignment
        )
        .unwrap();
        writeln!(
            output,
            "layout policy: {}",
            match self.layout_policy {
                SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1 =>
                    "entry-then-zero-fallthrough-then-nonzero-v1",
                SelectedFunctionLayoutPolicy::SingleEntryBlockV1 => "single-entry-block-v1",
                SelectedFunctionLayoutPolicy::StructuralUnitCallThenReturnSingleEntryBlockV1 =>
                    "structural-unit-call-then-return-single-entry-block-v1",
            }
        )
        .unwrap();
        writeln!(
            output,
            "scope: function-relative-fragments-with-validated-whole-function-exit-v1"
        )
        .unwrap();
        writeln!(output, "functions: {}", self.statistics.functions).unwrap();
        writeln!(output, "blocks: {}", self.statistics.blocks).unwrap();
        writeln!(output, "instructions: {}", self.statistics.instructions).unwrap();
        writeln!(output, "function-relative bytes: {}", self.statistics.bytes).unwrap();
        writeln!(
            output,
            "resolved conditional branches: {}",
            self.statistics.resolved_conditional_branches
        )
        .unwrap();
        writeln!(
            output,
            "structural functions: {}",
            self.statistics.structural_unit_functions
        )
        .unwrap();
        writeln!(
            output,
            "structural blocks: {}",
            self.statistics.structural_unit_blocks
        )
        .unwrap();
        writeln!(
            output,
            "structural instructions: {}",
            self.statistics.structural_unit_instructions
        )
        .unwrap();
        writeln!(
            output,
            "structural function-relative bytes: {}",
            self.statistics.structural_unit_bytes
        )
        .unwrap();
        writeln!(
            output,
            "unresolved internal-Machine fixups: {}",
            self.statistics.unresolved_internal_machine_fixups
        )
        .unwrap();
        writeln!(output, "frame: unavailable").unwrap();
        writeln!(output, "machine emission: unavailable").unwrap();
        writeln!(output, "section placement: unavailable").unwrap();
        writeln!(output, "symbols: unavailable").unwrap();
        writeln!(output, "object relocations: unavailable").unwrap();
        writeln!(output, "executable image: unavailable").unwrap();
        writeln!(output, "installation: unavailable").unwrap();
        writeln!(output, "publication: unavailable").unwrap();
        output
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationRealizationManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownSelectedLoweringCompletionStatus(u8),
    UnknownX86BranchRelaxationStatus(u8),
    UnknownAarch64CbnzFusionStatus(u8),
    UnknownAarch64MovnMaterializationStatus(u8),
    ConflictingPhysicalTransformations,
    UnknownArchitecture(u8),
    UnknownObjectFormat(u8),
    TargetLayoutOverflow,
    UnknownLayoutPolicy(u8),
    UnknownScope(u8),
    UnknownUnavailableStatus(u8),
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for FunctionRelativeOptimizationRealizationManifestDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid function-relative realization manifest: {self:?}"
        )
    }
}

impl std::error::Error for FunctionRelativeOptimizationRealizationManifestDecodeError {}
fn encode_manifest_content(manifest: &FunctionRelativeOptimizationRealizationManifest) -> Vec<u8> {
    let mut canonical = Vec::new();
    canonical.push(match manifest.stage {
        FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1 => 1,
    });
    canonical.extend_from_slice(&manifest.selections.bytes());
    canonical.extend_from_slice(&manifest.selected_lowering_selections.bytes());
    match manifest.selected_lowering_completion {
        Some(identity) => {
            canonical.push(1);
            canonical.extend_from_slice(&identity.bytes());
        }
        None => canonical.push(0),
    }
    canonical.extend_from_slice(&manifest.allocation_recovery_selections.bytes());
    for identity in [
        manifest.post_allocation_machine_selections.bytes(),
        manifest.function_relative_layout_selections.bytes(),
        manifest.pre_physical_manifest.bytes(),
        manifest.post_allocation_manifest.bytes(),
        manifest.selected.bytes(),
        manifest.pre_allocation_machine_effects.bytes(),
        manifest.post_allocation_machine.bytes(),
        manifest.baseline_pre_layout.bytes(),
        manifest.pre_layout.bytes(),
        manifest.baseline_resolved_layout.bytes(),
        manifest.resolved_layout.bytes(),
    ] {
        canonical.extend_from_slice(&identity);
    }
    match manifest.x86_branch_relaxation {
        Some(identity) => {
            canonical.push(1);
            canonical.extend_from_slice(&identity.bytes());
        }
        None => canonical.push(0),
    }
    match manifest.aarch64_cbnz_fusion {
        Some(identity) => {
            canonical.push(1);
            canonical.extend_from_slice(&identity.bytes());
        }
        None => canonical.push(0),
    }
    match manifest.aarch64_movn_materialization {
        Some(identity) => {
            canonical.push(1);
            canonical.extend_from_slice(&identity.bytes());
        }
        None => canonical.push(0),
    }
    canonical.extend_from_slice(&manifest.whole_function_exit_contract.bytes());
    encode_target(&mut canonical, manifest.target);
    canonical.push(match manifest.layout_policy {
        SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1 => 1,
        SelectedFunctionLayoutPolicy::SingleEntryBlockV1 => 2,
        SelectedFunctionLayoutPolicy::StructuralUnitCallThenReturnSingleEntryBlockV1 => 3,
    });
    canonical.push(match manifest.scope {
        FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1 => 1,
    });
    for value in [
        manifest.statistics.functions,
        manifest.statistics.blocks,
        manifest.statistics.instructions,
        manifest.statistics.bytes,
        manifest.statistics.resolved_conditional_branches,
        manifest.statistics.structural_unit_functions,
        manifest.statistics.structural_unit_blocks,
        manifest.statistics.structural_unit_instructions,
        manifest.statistics.structural_unit_bytes,
        manifest.statistics.unresolved_internal_machine_fixups,
    ] {
        canonical.extend_from_slice(&value.to_le_bytes());
    }
    for unavailable in [
        manifest.frame,
        manifest.machine_emission,
        manifest.section_placement,
        manifest.symbols,
        manifest.object_relocations,
        manifest.executable_image,
        manifest.installation,
        manifest.publication,
    ] {
        canonical.push(match unavailable {
            FunctionRelativeOptimizationUnavailableData::Unavailable => 1,
        });
    }
    canonical
}

fn encode_target(bytes: &mut Vec<u8>, target: NativeTarget) {
    bytes.push(match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    });
    bytes.push(match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    });
    encode_usize(bytes, target.pointer_size);
    encode_usize(bytes, target.pointer_alignment);
}

const fn architecture_name(architecture: Architecture) -> &'static str {
    match architecture {
        Architecture::Aarch64 => "aarch64",
        Architecture::X86_64 => "x86_64",
    }
}

const fn object_format_name(object_format: ObjectFormat) -> &'static str {
    match object_format {
        ObjectFormat::Elf => "elf",
        ObjectFormat::MachO => "macho",
        ObjectFormat::Coff => "coff",
    }
}

fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, FunctionRelativeOptimizationRealizationManifestDecodeError> {
    let architecture = match cursor.byte()? {
        1 => Architecture::Aarch64,
        2 => Architecture::X86_64,
        tag => {
            return Err(
                FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownArchitecture(
                    tag,
                ),
            );
        }
    };
    let object_format = match cursor.byte()? {
        1 => ObjectFormat::Elf,
        2 => ObjectFormat::MachO,
        3 => ObjectFormat::Coff,
        tag => {
            return Err(
                FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownObjectFormat(
                    tag,
                ),
            );
        }
    };
    let pointer_size = usize::try_from(u64::from_le_bytes(cursor.array()?)).map_err(|_| {
        FunctionRelativeOptimizationRealizationManifestDecodeError::TargetLayoutOverflow
    })?;
    let pointer_alignment = usize::try_from(u64::from_le_bytes(cursor.array()?)).map_err(|_| {
        FunctionRelativeOptimizationRealizationManifestDecodeError::TargetLayoutOverflow
    })?;
    Ok(NativeTarget {
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
    })
}

fn decode_unavailable(
    cursor: &mut Cursor<'_>,
) -> Result<
    FunctionRelativeOptimizationUnavailableData,
    FunctionRelativeOptimizationRealizationManifestDecodeError,
> {
    match cursor.byte()? {
        1 => Ok(FunctionRelativeOptimizationUnavailableData::Unavailable),
        tag => Err(
            FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownUnavailableStatus(
                tag,
            ),
        ),
    }
}

fn encode_usize(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("function-relative realization value fits u64")
            .to_le_bytes(),
    );
}

struct Cursor<'encoded> {
    encoded: &'encoded [u8],
    offset: usize,
}

impl<'encoded> Cursor<'encoded> {
    const fn new(encoded: &'encoded [u8]) -> Self {
        Self { encoded, offset: 0 }
    }

    fn take(
        &mut self,
        length: usize,
    ) -> Result<&'encoded [u8], FunctionRelativeOptimizationRealizationManifestDecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(FunctionRelativeOptimizationRealizationManifestDecodeError::Truncated)?;
        let value = self
            .encoded
            .get(self.offset..end)
            .ok_or(FunctionRelativeOptimizationRealizationManifestDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], FunctionRelativeOptimizationRealizationManifestDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| FunctionRelativeOptimizationRealizationManifestDecodeError::Truncated)
    }

    fn byte(&mut self) -> Result<u8, FunctionRelativeOptimizationRealizationManifestDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    fn remaining(&self) -> usize {
        self.encoded.len() - self.offset
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}

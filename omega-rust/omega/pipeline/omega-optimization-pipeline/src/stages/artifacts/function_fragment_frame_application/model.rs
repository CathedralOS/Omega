use omega_machine_code::FunctionFragmentEmissionPlan;
use omega_optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
};
use omega_selected_instructions::{SelectedBlockId, SelectedInstructionId};
use psi_core::{EdgeId, MachineId};

use crate::{StagedOptimizedFunctionFragmentEmission, TargetFrameProtocolEncodingIdentity};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionFragmentFrameApplicationIdentity([u8; 32]);

impl FunctionFragmentFrameApplicationIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionAppliedFrameEpilogue {
    pub block: SelectedBlockId,
    pub return_instruction: SelectedInstructionId,
    pub psi_return_edge: EdgeId,
    pub function_offset: u64,
    pub byte_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionAppliedFrameProtocol {
    pub machine: MachineId,
    pub prologue_function_offset: u64,
    pub prologue_byte_count: u64,
    pub epilogues: Vec<FunctionAppliedFrameEpilogue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentFrameApplication {
    pub identity: FunctionFragmentFrameApplicationIdentity,
    pub source_fragment_manifest: FunctionFragmentEmissionManifestIdentity,
    pub source_fragments: FunctionFragmentEmissionIdentity,
    pub frame_protocol: TargetFrameProtocolEncodingIdentity,
    pub functions: Vec<FunctionAppliedFrameProtocol>,
    pub fragments: FunctionFragmentEmissionPlan,
}

impl FunctionFragmentFrameApplication {
    pub fn recomputed_identity(&self) -> FunctionFragmentFrameApplicationIdentity {
        super::function_fragment_frame_application_identity(self)
    }
}

#[derive(Debug)]
#[must_use = "frame-applied fragments retain both selected-fragment and target-protocol custody"]
pub struct StagedFunctionFragmentFrameApplication {
    pub(super) source: StagedOptimizedFunctionFragmentEmission,
    pub(super) application: FunctionFragmentFrameApplication,
    pub(super) receipt: FunctionFragmentFrameApplicationReceipt,
}

impl StagedFunctionFragmentFrameApplication {
    pub const fn source(&self) -> &StagedOptimizedFunctionFragmentEmission {
        &self.source
    }

    pub const fn application(&self) -> &FunctionFragmentFrameApplication {
        &self.application
    }

    pub const fn fragments(&self) -> &FunctionFragmentEmissionPlan {
        &self.application.fragments
    }

    pub const fn receipt(&self) -> FunctionFragmentFrameApplicationReceipt {
        self.receipt
    }

    #[cfg(test)]
    pub(crate) fn corrupt_first_epilogue_site_for_test(&mut self) {
        self.application.functions[0].epilogues[0].function_offset += 1;
        self.reseal_for_test();
    }

    #[cfg(test)]
    pub(crate) fn corrupt_first_branch_byte_for_test(&mut self) {
        let function = &mut self.application.fragments.functions[0];
        let row = function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find(|row| row.branch.is_some())
            .unwrap();
        let byte_index = row.bytes.len() - 1;
        row.bytes[byte_index] ^= 1;
        function.bytes[row.offset as usize + byte_index] ^= 1;
        self.application.fragments.identity = self.application.fragments.recomputed_identity();
        self.reseal_for_test();
    }

    #[cfg(test)]
    fn reseal_for_test(&mut self) {
        self.application.identity = self.application.recomputed_identity();
        self.receipt = seal(&self.application);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionFragmentFrameApplicationReceipt {
    identity: FunctionFragmentFrameApplicationIdentity,
    source_fragment_manifest: FunctionFragmentEmissionManifestIdentity,
    source_fragments: FunctionFragmentEmissionIdentity,
    frame_protocol: TargetFrameProtocolEncodingIdentity,
    fragments: FunctionFragmentEmissionIdentity,
    framed_function_count: usize,
    epilogue_application_count: usize,
}

impl FunctionFragmentFrameApplicationReceipt {
    pub const fn identity(self) -> FunctionFragmentFrameApplicationIdentity {
        self.identity
    }

    pub const fn source_fragment_manifest(self) -> FunctionFragmentEmissionManifestIdentity {
        self.source_fragment_manifest
    }

    pub const fn source_fragments(self) -> FunctionFragmentEmissionIdentity {
        self.source_fragments
    }

    pub const fn frame_protocol(self) -> TargetFrameProtocolEncodingIdentity {
        self.frame_protocol
    }

    pub const fn fragments(self) -> FunctionFragmentEmissionIdentity {
        self.fragments
    }

    pub const fn framed_function_count(self) -> usize {
        self.framed_function_count
    }

    pub const fn epilogue_application_count(self) -> usize {
        self.epilogue_application_count
    }
}

pub(super) fn seal(
    application: &FunctionFragmentFrameApplication,
) -> FunctionFragmentFrameApplicationReceipt {
    FunctionFragmentFrameApplicationReceipt {
        identity: application.recomputed_identity(),
        source_fragment_manifest: application.source_fragment_manifest,
        source_fragments: application.source_fragments,
        frame_protocol: application.frame_protocol,
        fragments: application.fragments.identity,
        framed_function_count: application
            .functions
            .iter()
            .filter(|row| {
                row.prologue_byte_count != 0
                    || row
                        .epilogues
                        .iter()
                        .any(|epilogue| epilogue.byte_count != 0)
            })
            .count(),
        epilogue_application_count: application
            .functions
            .iter()
            .map(|row| row.epilogues.len())
            .sum(),
    }
}

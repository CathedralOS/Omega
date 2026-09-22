//! Optimizer module role: stage group. Apply validated target frame protocol
//! bytes to already replayable ordinary function fragments.
//!
//! Every framed function receives one prologue and one epilogue at each return
//! site. Conditional-branch coordinates and bytes are replayed after insertion
//! with the target-owned encoders.

mod error;
mod validation;

pub use error::FunctionFragmentFrameApplicationError;
pub use machine_code::function_fragment_frame_application_identity;

use crate::{
    StagedOptimizedFunctionFragmentEmission, validate_optimized_function_fragment_emission,
};
pub use machine_code::{
    FunctionAppliedFrameEpilogue, FunctionAppliedFrameProtocol, FunctionFragmentFrameApplication,
    FunctionFragmentFrameApplicationIdentity,
};
use machine_code::{FunctionFragmentEmissionPlan, TargetFrameProtocolEncodingIdentity};
use optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
};

pub fn stage_function_fragment_frame_application(
    source: StagedOptimizedFunctionFragmentEmission,
) -> Result<StagedFunctionFragmentFrameApplication, FunctionFragmentFrameApplicationError> {
    validate_optimized_function_fragment_emission(&source)
        .map_err(FunctionFragmentFrameApplicationError::Source)?;
    let application = {
        let protocol = source.source().frame_protocol();
        crate::apply_frame_protocol_to_fragments(
            source.fragments(),
            source.manifest().record().identity,
            protocol,
            source.source().register_environment().physical(),
        )
        .map_err(FunctionFragmentFrameApplicationError::from)?
    };
    let receipt = seal(&application);
    let staged = StagedFunctionFragmentFrameApplication {
        source,
        application: std::sync::Arc::new(application),
        receipt,
    };
    validate_function_fragment_frame_application(&staged)?;
    Ok(staged)
}

pub fn validate_function_fragment_frame_application(
    staged: &StagedFunctionFragmentFrameApplication,
) -> Result<FunctionFragmentFrameApplicationReceipt, FunctionFragmentFrameApplicationError> {
    validation::validate(staged)
}

#[derive(Debug)]
#[must_use = "frame-applied fragments retain both selected-fragment and target-protocol custody"]
pub struct StagedFunctionFragmentFrameApplication {
    source: StagedOptimizedFunctionFragmentEmission,
    application: std::sync::Arc<FunctionFragmentFrameApplication>,
    receipt: FunctionFragmentFrameApplicationReceipt,
}

impl StagedFunctionFragmentFrameApplication {
    pub const fn source(&self) -> &StagedOptimizedFunctionFragmentEmission {
        &self.source
    }

    pub fn application(&self) -> &FunctionFragmentFrameApplication {
        &self.application
    }

    pub fn fragments(&self) -> &FunctionFragmentEmissionPlan {
        &self.application.fragments
    }

    pub fn shared_application(&self) -> std::sync::Arc<FunctionFragmentFrameApplication> {
        std::sync::Arc::clone(&self.application)
    }

    pub const fn receipt(&self) -> FunctionFragmentFrameApplicationReceipt {
        self.receipt
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_first_epilogue_site_for_test(&mut self) {
        std::sync::Arc::make_mut(&mut self.application).functions[0].epilogues[0]
            .function_offset += 1;
        self.reseal_for_test();
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_first_branch_byte_for_test(&mut self) {
        let application = std::sync::Arc::make_mut(&mut self.application);
        let function = &mut application.fragments.functions[0];
        let row = function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find(|row| row.branch.is_some())
            .unwrap();
        let byte_index = row.bytes.len() - 1;
        row.bytes[byte_index] ^= 1;
        function.bytes[row.offset as usize + byte_index] ^= 1;
        application.fragments.identity = application.fragments.recomputed_identity();
        self.reseal_for_test();
    }

    #[cfg(any(test, feature = "test-support"))]
    fn reseal_for_test(&mut self) {
        let application = std::sync::Arc::make_mut(&mut self.application);
        application.identity = application.recomputed_identity();
        self.receipt = seal(application);
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

fn seal(application: &FunctionFragmentFrameApplication) -> FunctionFragmentFrameApplicationReceipt {
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

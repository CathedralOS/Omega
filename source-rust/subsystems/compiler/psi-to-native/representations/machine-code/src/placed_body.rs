//! Current, framed native code and the records needed to interpret its bytes.

use crate::{
    FunctionFragmentFrameApplication, ResolvedMachineLayout, TargetFrameLayoutPlan,
    TargetFrameProtocolEncodingPlan,
};
use register_homes::AllocationPublicationFacts;
use register_model::ValidatedPhysicalRegisterModel;
use std::sync::Arc;

/// A placed body exposes per-instruction effects to object and image readers.
///
/// ```compile_fail,E0609
/// fn original_encoding(body: &machine_code::placed_body::NativeBodyReadback) {
///     let _ = body.encoding.rows();
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBodyReadback {
    pub physical: ValidatedPhysicalRegisterModel,
    pub instruction_effects: Arc<[PlacedInstructionEffects]>,
    pub layout: Arc<ResolvedMachineLayout>,
    pub frame: Arc<TargetFrameLayoutPlan>,
    pub protocol: Arc<TargetFrameProtocolEncodingPlan>,
    pub frame_application: Arc<FunctionFragmentFrameApplication>,
    pub allocation: Arc<AllocationPublicationFacts>,
}

/// Object and image checks need each instruction's decoded effects and owning
/// function. Footprints stay shared when several instructions have identical effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedInstructionEffects {
    pub machine: semantic_vocabulary::MachineId,
    pub instruction: selected_instructions::SelectedInstructionId,
    footprint: Option<Arc<crate::SelectedFormDecodedFootprint>>,
}

impl PlacedInstructionEffects {
    pub fn from_footprint(
        machine: semantic_vocabulary::MachineId,
        instruction: selected_instructions::SelectedInstructionId,
        store: &crate::SelectedFormEncodedRowStore,
        handle: crate::SelectedFormDecodedFootprintHandle,
    ) -> Self {
        Self {
            machine,
            instruction,
            footprint: store.share_footprint(handle),
        }
    }

    pub fn footprint(&self) -> Option<&crate::SelectedFormDecodedFootprint> {
        self.footprint.as_deref()
    }

    pub fn effects(&self) -> Option<&selected_instructions::MachineEncodedEffects> {
        self.footprint.as_ref().map(|footprint| &footprint.encoded)
    }
}

impl crate::exit_requirements::EncodedExitRequirements {
    pub fn placed_instruction_effects(&self) -> Arc<[PlacedInstructionEffects]> {
        self.functions
            .iter()
            .flat_map(|function| {
                function.blocks.iter().flat_map(move |block| {
                    block.instructions.iter().map(move |row| {
                        PlacedInstructionEffects::from_footprint(
                            function.machine,
                            row.instruction,
                            &self.encoding.decoded,
                            row.encoding.footprint,
                        )
                    })
                })
            })
            .collect()
    }
}

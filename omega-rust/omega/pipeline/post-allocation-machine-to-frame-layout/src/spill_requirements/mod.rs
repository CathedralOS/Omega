//! Optimizer module role: executable entrance. Non-authoritative spill-frame requirements.
//!
//! This join authenticates abstract spill-access custody against the selected
//! register environment, derives requirements, and admits them through an
//! independent replay. It chooses no frame layout or executable operation.

mod compute;
mod custody;
mod identity;
mod model;
mod replay;
mod validation;

pub use identity::non_authoritative_spill_frame_requirement_identity;
pub use model::*;
pub use validation::validate_non_authoritative_spill_frame_requirements;

#[cfg(test)]
pub(in crate::spill_requirements) use compute::derive_zero_access_requirement_for_test;
#[cfg(test)]
pub(in crate::spill_requirements) use replay::replay_zero_access_requirement_for_test;

use optimization_core::OptimizationWorkBudget;
use selected_instructions_to_register_homes::ValidatedAbstractSpillAccessConstraints;

use crate::ValidatedTargetRegisterEnvironment;

pub fn stage_non_authoritative_spill_frame_requirements(
    source: &ValidatedAbstractSpillAccessConstraints,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: NonAuthoritativeSpillFrameRequirementPolicy,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedNonAuthoritativeSpillFrameRequirements, SpillFrameRequirementError> {
    let plan = compute::derive(source, environment, policy, budget)?;
    validate_non_authoritative_spill_frame_requirements(source, environment, plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use target_to_register_environment::FrameAbiPreservationConvention;

    #[test]
    fn independent_zero_access_rows_retain_neutral_alignment_without_inventing_a_frame() {
        let machine = semantic_vocabulary::MachineId::new(41_991).unwrap();
        let direct = derive_zero_access_requirement_for_test(machine);
        let replayed = replay_zero_access_requirement_for_test(machine);
        assert_eq!(direct, replayed);
        assert_eq!(direct.abstract_spill_area_bytes, 0);
        assert_eq!(direct.abstract_spill_area_alignment, 1);
        assert_eq!(
            direct.abi_preservation_convention,
            FrameAbiPreservationConvention::SystemVAMD64
        );
        assert_eq!(direct.abi_stack_alignment, 16);
        assert_eq!(direct.abi_red_zone_capacity_bytes, 128);
    }
}

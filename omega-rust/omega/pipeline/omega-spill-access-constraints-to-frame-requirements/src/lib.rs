#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Abstract spill-access constraints to validated frame requirements.
//!
//! The executable entrance derives abstract spill-area and target ABI
//! requirements, then independently replays them against exact source custody.
//! It chooses no concrete frame layout, instruction, unwind, or emission fact.

mod frame_requirements;

use omega_target_to_register_environment::ValidatedTargetRegisterEnvironment;

pub use frame_requirements::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_zero_access_rows_retain_neutral_alignment_without_inventing_a_frame() {
        let machine = psi_core::MachineId::new(41_991).unwrap();
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

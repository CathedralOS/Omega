//! Assess a borrowed interpretation without observing or changing content.
//! The selected supply and actual provider allocation must independently admit
//! the complete demand. Only the latter bounds physical authority; neither
//! compatibility result establishes vacancy, residency or a storage loan.

use super::initialization::InitializationRejection;
use super::{ProviderStorageBinding, RuntimeProviderStorage};
use crate::errors::{TerminalInterpretError, malformed};
use access_plans::{
    PlacementResourceCompatibility, ValidatedPlacementPlan, ValidatedResourceProfile,
};

impl RuntimeProviderStorage {
    pub(crate) fn prepare_view(
        &self,
        binding: ProviderStorageBinding,
        supplied: &ValidatedResourceProfile,
        placement: &ValidatedPlacementPlan,
    ) -> Result<
        Result<PlacementResourceCompatibility, InitializationRejection>,
        TerminalInterpretError,
    > {
        if !self.binding_is_live(binding) {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "borrowed placement lost its live original backing",
            ));
        }
        let (allocation, profile, base) = self.live_provider_allocation(&binding)?;
        let required = placement.layout().size.ok_or_else(malformed)?;
        if required > binding.length {
            return Ok(Err(InitializationRejection::RangeTooSmall {
                required,
                available: binding.length,
            }));
        }
        let loan = allocation
            .extent
            .loan(binding.offset, binding.length)
            .map_err(|reason| TerminalInterpretError::PlacedReference(reason.to_string()))?;
        let admitted = profile
            .restrict_to_loan(&loan)
            .map_err(|reason| TerminalInterpretError::PlacedReference(reason.to_string()))?;
        let mut actual = None;
        for profile in [supplied, &admitted] {
            let Ok(compatibility) = access_plans::validate_placement_resources(placement, profile)
            else {
                return Ok(Err(InitializationRejection::IncompatibleSupply));
            };
            let congruence = compatibility.base_congruence();
            if !congruence.admits(base) {
                return Ok(Err(InitializationRejection::Misaligned {
                    modulus: congruence.modulus(),
                    residue: congruence.residue(),
                }));
            }
            actual = Some(compatibility);
        }
        Ok(Ok(actual.ok_or_else(malformed)?))
    }
}

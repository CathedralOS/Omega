//! Join a staged whole-value image to its exact admitted allocation window.
//!
//! The selected profile call describes the requested supply; the embedding
//! allocation's admission bounds its physical authority. Both must support the
//! complete write and subsequent view. A selected provider cannot amplify the
//! allocation's rights by returning more permissive ordinary profile data.
//! Preparation changes no bytes, descriptor, qualification or custody. Its
//! dynamic rejection is intended for the ordinary Rejected result carrying
//! the unchanged inputs, not a host error or a replacement extent.

use super::{ProviderStorageBinding, ProviderStorageSlot, RuntimeProviderStorage};
use crate::errors::{TerminalInterpretError, malformed};
use access_plans::{
    InitializationResourceCompatibility, ValidatedPlacementPlan, ValidatedResourceProfile,
};

#[derive(Debug)]
pub(crate) enum InitializationRejection {
    RangeTooSmall { required: u64, available: u64 },
    Misaligned { modulus: u64, residue: u64 },
    IncompatibleSupply,
}

/// This is only staged storage. Publishing Resident additionally requires the
/// complete original value's ownership transaction. There is deliberately no
/// standalone write method that could commit bytes ahead of that transaction.
pub(crate) struct PreparedProviderImage {
    pub(super) binding: ProviderStorageBinding,
    pub(super) image: Vec<u8>,
    /// Subsequent accesses specialize from actual admitted backing, never
    /// from the selected call's possibly more permissive profile description.
    pub(super) compatibility: InitializationResourceCompatibility,
}

impl RuntimeProviderStorage {
    pub(crate) fn prepare_initial_image(
        &mut self,
        binding: ProviderStorageBinding,
        supplied: ValidatedResourceProfile,
        placement: &ValidatedPlacementPlan,
        image: Vec<u8>,
    ) -> Result<Result<PreparedProviderImage, InitializationRejection>, TerminalInterpretError>
    {
        let required = placement.layout().size.ok_or_else(malformed)?;
        if u64::try_from(image.len()).ok() != Some(required) {
            return Err(malformed());
        }
        let ProviderStorageSlot::Live {
            allocation,
            profile,
            ..
        } = self.allocations.get_mut(binding.allocation)
        else {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "initialization allocation retired before staging",
            ));
        };
        let end = binding
            .offset
            .checked_add(binding.length)
            .ok_or_else(malformed)?;
        if end > allocation.extent.length()
            || usize::try_from(end)
                .ok()
                .is_none_or(|end| end > allocation.bytes.len())
        {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "initialization window exceeds its original allocation",
            ));
        }
        if required > binding.length {
            return Ok(Err(InitializationRejection::RangeTooSmall {
                required,
                available: binding.length,
            }));
        }
        let Ok(compatibility) =
            access_plans::validate_initialization_resources(placement, &supplied)
        else {
            return Ok(Err(InitializationRejection::IncompatibleSupply));
        };
        let base = allocation
            .extent
            .base()
            .checked_add(binding.offset)
            .ok_or_else(malformed)?;
        let congruence = compatibility.placement().base_congruence();
        if !congruence.admits(base) {
            return Ok(Err(InitializationRejection::Misaligned {
                modulus: congruence.modulus(),
                residue: congruence.residue(),
            }));
        }
        let Ok(loan) = allocation.extent.loan_mut(binding.offset, binding.length) else {
            return Ok(Err(InitializationRejection::IncompatibleSupply));
        };
        let Ok(admitted) = profile.restrict_to_loan(&loan) else {
            return Ok(Err(InitializationRejection::IncompatibleSupply));
        };
        let Ok(compatibility) =
            access_plans::validate_initialization_resources(placement, &admitted)
        else {
            return Ok(Err(InitializationRejection::IncompatibleSupply));
        };
        // Each profile may select a different implementation of the same
        // declared access. Both impose alignment, but only the admitted
        // allocation decides which physical operation is available. For
        // example, an External read cannot become a Stable read merely
        // because the selected provider described Stable supply.
        let congruence = compatibility.placement().base_congruence();
        if !congruence.admits(base) {
            return Ok(Err(InitializationRejection::Misaligned {
                modulus: congruence.modulus(),
                residue: congruence.residue(),
            }));
        }
        Ok(Ok(PreparedProviderImage {
            binding,
            image,
            compatibility,
        }))
    }
}

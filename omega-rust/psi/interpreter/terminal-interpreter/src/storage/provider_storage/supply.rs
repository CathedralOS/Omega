//! Join dynamically supplied host memory to an already established guest grant.
//! The request is produced only at a selected resource-profile crossing. The
//! resolver supplies actual retained storage and its admitted ceiling; it does
//! not establish the guest's qualification, ownership, or resident contents.

use super::backing::ProviderBytes;
use super::{
    ProviderByteAllocation, ProviderStorageBinding, ProviderStorageSlot, TerminalProviderBacking,
};
use crate::boundary::values::StructuralRuntimePlace;
use crate::engine::execution::TerminalExecution;
use crate::{TerminalEffectRejection, TerminalInterpretError};
use access_plans::AdmittedResourceProfile;
use extents::Extent;
use semantic_vocabulary::{BoundaryMachineId, MachineId};

pub struct TerminalProviderStorageRequest {
    pub(crate) base: u64,
    pub(crate) length: u64,
    pub(crate) boundary: BoundaryMachineId,
    pub(crate) provider: MachineId,
}

impl TerminalProviderStorageRequest {
    pub fn base(&self) -> u64 {
        self.base
    }
    pub fn length(&self) -> u64 {
        self.length
    }
    pub fn boundary(&self) -> BoundaryMachineId {
        self.boundary
    }
    pub fn provider(&self) -> MachineId {
        self.provider
    }
}

/// Actual complete allocation, independently of the requested subrange. The
/// host keeps the same backing handle for retirement and must not manufacture
/// storage merely because a guest presented numeric geometry.
pub struct TerminalProviderStorageSupply {
    pub extent: Extent,
    pub profile: AdmittedResourceProfile,
    pub backing: TerminalProviderBacking,
}

pub trait TerminalProviderStorageResolver {
    fn resolve(
        &self,
        request: &TerminalProviderStorageRequest,
    ) -> Result<Option<TerminalProviderStorageSupply>, TerminalEffectRejection>;
}

impl TerminalExecution<'_> {
    pub(crate) fn bind_dynamic_provider_storage(
        &mut self,
        home: StructuralRuntimePlace,
        request: &TerminalProviderStorageRequest,
        supply: TerminalProviderStorageSupply,
    ) -> Result<ProviderStorageBinding, TerminalInterpretError> {
        let refusal = |reason| TerminalInterpretError::ContentTransformationMismatch(reason);
        let end = request
            .base
            .checked_add(request.length)
            .ok_or_else(|| refusal("provider storage request overflows its address space"))?;
        if supply.extent.provider_issuance().is_none()
            || request.base < supply.extent.base()
            || end > supply.extent.end()
            || usize::try_from(supply.extent.length()).ok()
                != Some(
                    supply
                        .backing
                        .lock()
                        .map_err(TerminalInterpretError::ResidentStorage)?
                        .len(),
                )
        {
            return Err(refusal(
                "dynamic provider storage differs from its actual allocation",
            ));
        }
        let offset = request.base - supply.extent.base();
        let loan = supply
            .extent
            .loan(offset, request.length)
            .map_err(|error| TerminalInterpretError::AccessReference(error.to_string()))?;
        supply
            .profile
            .restrict_to_loan(&loan)
            .map_err(|error| TerminalInterpretError::AccessReference(error.to_string()))?;

        let mut existing_handle = None;
        for (handle, row) in self.provider_storage.allocations.iter() {
            let ProviderStorageSlot::Live {
                allocation,
                profile,
                ..
            } = row
            else {
                continue;
            };
            if let ProviderBytes::Host(backing) = &allocation.bytes
                && backing.same_allocation(&supply.backing)
            {
                if allocation.extent != supply.extent
                    || profile.receipt() != supply.profile.receipt()
                    || profile.profile() != supply.profile.profile()
                {
                    return Err(refusal(
                        "provider changed the admission of an existing allocation",
                    ));
                }
                existing_handle = Some(handle);
            } else if allocation.extent.address_space() == supply.extent.address_space()
                && allocation.extent.era() == supply.extent.era()
                && allocation.extent.base() < supply.extent.end()
                && supply.extent.base() < allocation.extent.end()
            {
                return Err(refusal(
                    "different provider allocations overlap in one address space",
                ));
            }
        }
        // This catches inconsistent overlapping owners in this execution.
        // Cross-component exclusivity remains an obligation of the admitted
        // issuer; a shared host backing handle is not a global custody ledger.
        if let Some(allocation) = existing_handle {
            for (other, binding) in self.provider_storage.descriptors.iter() {
                if binding.allocation == allocation
                    && *other != home
                    && binding.offset < offset + request.length
                    && offset < binding.offset + binding.length
                    && self.is_live_provider_descriptor(other, binding)
                {
                    return Err(refusal(
                        "dynamic provider grant overlaps another live owner",
                    ));
                }
            }
        }
        let lease = self
            .account
            .as_ref()
            .filter(|_| existing_handle.is_none())
            .map(|account| account.reserve_storage(1, 0))
            .transpose()
            .map_err(TerminalInterpretError::Storage)?;
        let binding = self.provider_storage.descriptors.insert_vacant(
            home,
            ProviderStorageBinding {
                allocation: existing_handle.unwrap_or_default(),
                offset,
                length: request.length,
                resident: arena::Handle::default(),
            },
        )?;
        if existing_handle.is_none() {
            binding.allocation =
                self.provider_storage
                    .allocations
                    .insert(ProviderStorageSlot::Live {
                        allocation: ProviderByteAllocation {
                            extent: supply.extent,
                            bytes: ProviderBytes::Host(supply.backing),
                        },
                        profile: supply.profile,
                        _lease: lease,
                    });
        }
        Ok(*binding)
    }
}

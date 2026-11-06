//! Actual provider-supplied bytes, distinct from the Extent descriptor's cells.
//!
//! This owner consumes both an already admitted extent and its supplied storage.
//! Moving it moves neither the allocation nor its bytes. A numeric base is never
//! dereferenced as a host pointer. At a selected profile crossing, an explicitly
//! installed host resolver may join checked geometry to storage it already owns;
//! geometry alone creates neither storage nor guest authority. Shared host handles
//! retain the same bytes and retirement state, not copies or copy-on-write data.
//! Each live arena record and descriptor/profile row owns a logical cell.
//! This accounts records, not the host-supplied byte buffer's allocation or
//! compiler-owned image scratch payloads; it is not a bound on RSS.
//!
//! This is the byte-writing part of lifecycle execution, not its custody gate.
//! The caller must bind the selected provider occurrence and current Vacant
//! claim before writing, then transfer the complete source value before
//! publishing Resident. No method here issues a qualification or source claim.

use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::engine::execution::TerminalExecution;
use crate::storage::record::owned_value::StagedOwnedValue;
use crate::storage::runtime_rows::RuntimeRows;
use access_plans::{AdmittedResourceProfile, InitializationResourceCompatibility};
use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};
use extents::Extent;
use terminal_psi::TerminalAccessLifecyclePlan;

mod backing;
use backing::ProviderBytes;
pub use backing::TerminalProviderBacking;
mod entry;
mod supply;
pub use supply::{
    TerminalProviderStorageRequest, TerminalProviderStorageResolver, TerminalProviderStorageSupply,
};
mod empty_run;
mod extraction;
pub(crate) mod field_access;
pub(crate) mod initialization;
mod lifecycle;
mod profile_values;
pub(crate) mod profiles;
pub(crate) mod value_image;
pub(crate) mod view;
pub use entry::{
    TerminalProviderStorageInput, TerminalProviderStorageRejection, TerminalReturnedProviderStorage,
};

/// Descriptor relocation copies this association, not the allocation. Live
/// source claims and loans remain the authorization for every operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProviderStorageBinding {
    pub(crate) allocation: arena::Handle<ProviderStorageSlot>,
    pub(crate) offset: u64,
    pub(crate) length: u64,
    /// Zero denotes no retained resident payload, not a vacancy proof.
    /// Initialization separately requires the source's exact Vacant contract;
    /// external views establish neither vacancy nor resident custody.
    /// Relocation carries this handle unchanged; neither an address nor a
    /// source qualification can discover its payload.
    pub(crate) resident: arena::Handle<ResidentStorageSlot>,
}

#[derive(Default)]
pub(crate) enum ResidentStorageSlot {
    #[default]
    Empty,
    Live {
        window: ProviderStorageBinding,
        initialized_by: semantic_vocabulary::BoundaryMachineId,
        custody: crate::storage::record::owned_value::StagedOwnedValue,
        compatibility: access_plans::InitializationResourceCompatibility,
        _lease: Option<EvaluationStorageLease>,
    },
}

#[derive(Debug, Default)]
pub(crate) enum ProviderStorageSlot {
    #[default]
    Empty,
    Live {
        allocation: ProviderByteAllocation,
        profile: AdmittedResourceProfile,
        _lease: Option<EvaluationStorageLease>,
    },
}

pub(crate) struct RuntimeProviderStorage {
    pub(crate) allocations: arena::Arena<ProviderStorageSlot>,
    pub(crate) residents: arena::Arena<ResidentStorageSlot>,
    pub(crate) descriptors:
        RuntimeRows<crate::boundary::values::StructuralRuntimePlace, ProviderStorageBinding>,
    pub(crate) completed_profiles: RuntimeRows<
        crate::boundary::values::StructuralRuntimePlace,
        profiles::CompletedResourceProfile,
    >,
}

impl RuntimeProviderStorage {
    pub(crate) fn new(account: Option<&EvaluationInvocationAccount>) -> Self {
        Self {
            allocations: Default::default(),
            residents: Default::default(),
            descriptors: RuntimeRows::new(account),
            completed_profiles: RuntimeRows::new(account),
        }
    }

    /// Backing liveness is independent of where its enclosing custody lives:
    /// an ordinary frame claim or a complete retained resident value. Callers
    /// still establish that ownership before transporting this association.
    pub(crate) fn binding_is_live(&self, binding: ProviderStorageBinding) -> bool {
        let ProviderStorageSlot::Live { allocation, .. } = self.allocations.get(binding.allocation)
        else {
            return false;
        };
        if binding
            .offset
            .checked_add(binding.length)
            .is_none_or(|end| end > allocation.extent.length())
        {
            return false;
        }
        if binding.resident != arena::Handle::default() {
            let ResidentStorageSlot::Live { window, .. } = self.residents.get(binding.resident)
            else {
                return false;
            };
            if *window
                != (ProviderStorageBinding {
                    resident: arena::Handle::default(),
                    ..binding
                })
            {
                return false;
            }
        }
        true
    }

    /// The live allocation slot a binding names, with the profile admitted for
    /// its supply and the absolute base of its window. Geometry, loans and
    /// authority checks stay with each caller's operation.
    pub(crate) fn live_provider_allocation(
        &self,
        binding: &ProviderStorageBinding,
    ) -> Result<(&ProviderByteAllocation, &AdmittedResourceProfile, u64), TerminalInterpretError>
    {
        let ProviderStorageSlot::Live {
            allocation,
            profile,
            ..
        } = self.allocations.get(binding.allocation)
        else {
            return Err(malformed());
        };
        let base = allocation
            .extent
            .base()
            .checked_add(binding.offset)
            .ok_or_else(malformed)?;
        Ok((allocation, profile, base))
    }
}

impl TerminalExecution<'_> {
    /// The live resident custody a binding names, with the lifecycle plan row
    /// its initialization boundary declares. Each caller compares the plan and
    /// custody against its own operation's contract; a retired slot or an
    /// unknown initialization boundary is malformed private runtime state.
    pub(crate) fn live_resident_custody(
        &self,
        resident: arena::Handle<ResidentStorageSlot>,
    ) -> Result<
        (
            &TerminalAccessLifecyclePlan,
            &StagedOwnedValue,
            &InitializationResourceCompatibility,
        ),
        TerminalInterpretError,
    > {
        let ResidentStorageSlot::Live {
            initialized_by,
            custody,
            compatibility,
            ..
        } = self.provider_storage.residents.get(resident)
        else {
            return Err(malformed());
        };
        let initialized = self
            .program
            .access_lifecycle_plans()
            .iter()
            .find(|plan| plan.boundary == *initialized_by)
            .ok_or_else(malformed)?;
        Ok((initialized, custody, compatibility))
    }
}

#[derive(Debug)]
pub(crate) struct ProviderByteAllocation {
    extent: Extent,
    bytes: ProviderBytes,
}

impl ProviderByteAllocation {
    pub(crate) fn extent(&self) -> &Extent {
        &self.extent
    }
}

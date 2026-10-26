//! Actual provider-supplied bytes, distinct from the Extent descriptor's cells.
//!
//! This owner consumes both an already admitted extent and its supplied storage.
//! Moving it moves neither the allocation nor its bytes. The numeric base is
//! checked geometry, never a host pointer or a key used to discover an allocation.
//! Unlike immutable byte values, provider storage has no clone or copy-on-write
//! operation.
//! Each live arena record and descriptor/profile row owns a logical cell.
//! This accounts records, not the host-supplied byte buffer's allocation or
//! compiler-owned image scratch payloads; it is not a bound on RSS.
//!
//! This is the byte-writing part of lifecycle execution, not its custody gate.
//! The caller must bind the selected provider occurrence and current Vacant
//! claim before writing, then transfer the complete source value before
//! publishing Resident. No method here issues a qualification or source claim.

use crate::runtime_rows::RuntimeRows;
use access_plans::AdmittedResourceProfile;
use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};
use extents::Extent;

mod entry;
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
        custody: crate::record::owned_value::StagedOwnedValue,
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
        RuntimeRows<crate::values::StructuralRuntimePlace, ProviderStorageBinding>,
    pub(crate) completed_profiles:
        RuntimeRows<crate::values::StructuralRuntimePlace, profiles::CompletedResourceProfile>,
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
}

#[derive(Debug)]
pub(crate) struct ProviderByteAllocation {
    extent: Extent,
    bytes: Vec<u8>,
}

/// Admission failure returns both owned inputs. In particular, a bad byte
/// capacity cannot silently discard the provider's linear extent.
#[derive(Debug)]
pub(crate) struct ProviderByteAllocationRejection {
    pub(crate) extent: Extent,
    pub(crate) bytes: Vec<u8>,
    pub(crate) reason: &'static str,
}

impl ProviderByteAllocation {
    /// The admitted provider supplies the allocation explicitly. This does
    /// not allocate storage from a numeric address or establish vacant content.
    pub(crate) fn from_admitted_provider(
        extent: Extent,
        bytes: Vec<u8>,
    ) -> Result<Self, ProviderByteAllocationRejection> {
        let reason = if extent.provider_issuance().is_none() {
            Some("provider byte storage requires an admitted provider-issued extent")
        } else if usize::try_from(extent.length()).ok() != Some(bytes.len()) {
            Some("provider byte storage must cover the exact complete extent")
        } else {
            None
        };
        if let Some(reason) = reason {
            return Err(ProviderByteAllocationRejection {
                extent,
                bytes,
                reason,
            });
        }
        Ok(Self { extent, bytes })
    }

    pub(crate) fn extent(&self) -> &Extent {
        &self.extent
    }

    /// Return the exact original extent and allocation to the completion owner.
    /// This performs no release, vacancy transition, or resident extraction.
    pub(crate) fn into_parts(self) -> (Extent, Vec<u8>) {
        (self.extent, self.bytes)
    }
}

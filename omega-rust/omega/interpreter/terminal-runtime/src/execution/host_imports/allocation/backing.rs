// Resolve checked `Extent` requests against live `AllocationValues`
// (`AllocationStorage`). Receipt identities are scoped to this host store: the
// allocating import is the issuer and its never-recycled allocation address
// identifies the issuance without minting any guest domain or ownership claim.

use super::AllocationValues;
use access_plans::{
    AtomicCapability, BoundaryReach, ExternalCapability, PeerWritability, ResourceProfile,
    ResourceProfileGrant, ResourceProfileReceiptId, ResourceRegion, StableCapability,
};
use extents::{
    AddressSpaceId, ExtentLineageId, ExtentProvenanceId, ExtentProviderIssuance, ExtentRightId,
    ExtentRights, ExtentRootGrant, MappingEraId,
};
use std::sync::{Arc, Mutex};
use terminal_interpreter::{
    TerminalEffectRejection, TerminalProviderStorageRequest, TerminalProviderStorageResolver,
    TerminalProviderStorageSupply,
};

pub(crate) struct AllocationStorage {
    pub(in crate::execution::host_imports) values: Arc<Mutex<AllocationValues>>,
}

impl TerminalProviderStorageResolver for AllocationStorage {
    fn resolve(
        &self,
        request: &TerminalProviderStorageRequest,
    ) -> Result<Option<TerminalProviderStorageSupply>, TerminalEffectRejection> {
        self.resolve_range(request.base(), request.length())
    }
}

impl AllocationStorage {
    fn resolve_range(
        &self,
        base: u64,
        length: u64,
    ) -> Result<Option<TerminalProviderStorageSupply>, TerminalEffectRejection> {
        let reject =
            |error: &dyn std::fmt::Display| TerminalEffectRejection::new(error.to_string());
        if base.checked_add(length).is_none() {
            return Err(TerminalEffectRejection::new(
                "allocation backing request overflows",
            ));
        }
        let values = self
            .values
            .lock()
            .map_err(|_| TerminalEffectRejection::new("allocation lifecycle store was poisoned"))?;
        let Some(allocation) = values.backing_for_range(base, length) else {
            return Ok(None);
        };
        if u64::try_from(allocation.backing.byte_count()?).ok() != Some(allocation.length) {
            return Err(TerminalEffectRejection::new(
                "allocation backing differs from its retained extent",
            ));
        }
        let id = allocation.address;
        let issuer = allocation.issuer;
        let issuance = ExtentProviderIssuance::from_normalized_identities([
            id, id, issuer, id, id, id, id, issuer, issuer, id, issuer, id, issuer,
        ])
        .map_err(|error| reject(&error))?;
        let extent = ExtentRootGrant::from_admitted_provider(
            issuance,
            ExtentLineageId::from_normalized_identity(id).map_err(|error| reject(&error))?,
            AddressSpaceId::from_normalized_identity(1).map_err(|error| reject(&error))?,
            ExtentRights::from_normalized_identities([
                ExtentRightId::from_normalized_identity(1).map_err(|error| reject(&error))?
            ]),
            ExtentProvenanceId::from_normalized_identity(id).map_err(|error| reject(&error))?,
            MappingEraId::from_normalized_identity(1).map_err(|error| reject(&error))?,
        )
        .mint(allocation.address, allocation.length)
        .map_err(|error| reject(error.diagnostic()))?;
        let profile = ResourceProfileGrant::from_admitted_provider(
            ResourceProfileReceiptId::from_normalized_identity(id)
                .map_err(|error| reject(&error))?,
            &extent,
            extent.rights().clone(),
            BoundaryReach::default(),
        )
        .map_err(|error| reject(&error))?
        .admit(ResourceProfile {
            regions: vec![ResourceRegion {
                offset: 0,
                length: allocation.length,
                peer: PeerWritability::Exclusive,
                stable: StableCapability::ReadWrite,
                external: ExternalCapability::None,
                atomic: AtomicCapability::None,
                reach: BoundaryReach::default(),
            }],
        })
        .map_err(|error| reject(error.diagnostic()))?;
        Ok(Some(TerminalProviderStorageSupply {
            extent,
            profile,
            backing: allocation.backing.clone(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::{AllocationStorage, AllocationValues};
    use std::sync::{Arc, Mutex};

    #[test]
    fn resolve_only_retained_live_ranges_and_preserve_physical_identity() {
        let values = Arc::new(Mutex::new(AllocationValues::default()));
        let address = values.lock().unwrap().allocate(1, 32, 7).unwrap();
        let resolver = AllocationStorage {
            values: values.clone(),
        };
        assert!(resolver.resolve_range(address + 32, 1).unwrap().is_none());
        assert!(resolver.resolve_range(1, 8).unwrap().is_none());
        assert!(resolver.resolve_range(u64::MAX, 1).is_err());
        let first = resolver.resolve_range(address, 16).unwrap().unwrap();
        let second = resolver.resolve_range(address + 16, 16).unwrap().unwrap();
        assert_eq!(first.extent, second.extent);
        assert_eq!(first.extent.length(), 32);
        assert_eq!(first.profile.profile(), second.profile.profile());
        values.lock().unwrap().release(address).unwrap();
        assert!(first.backing.byte_count().is_err());
        assert!(second.backing.byte_count().is_err());
        assert!(resolver.resolve_range(address, 16).is_err());
    }
}

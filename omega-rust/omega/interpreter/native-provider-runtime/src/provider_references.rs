//! Live provider referents are captured by a normal call inside the worker.
//! The sponsor receives an occurrence handle, never an address it can read.
//! These physical handles do not establish semantic permission, lifetime or
//! disjointness; the host joins them to the exact admitted boundary result.

use crate::{
    NativeAdapterId, NativeArgument, NativeProviderError, NativeProviderProcess,
    wire::{Cursor, Packet, invalid},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeResultStorage {
    Value,
    SharedProviderReference { byte_size: usize, alignment: usize },
}

/// One result occurrence in an exact provider era and installed adapter.
/// Distinct occurrences may denote the same external storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeProviderReference {
    pub(crate) era: u64,
    pub(crate) adapter: u64,
    pub(crate) occurrence: u64,
}

pub(crate) struct ProviderReferenceResult {
    pub(crate) adapter: u64,
    pub(crate) storage: NativeResultStorage,
}

pub(crate) fn occurrence_index(occurrence: u64) -> std::io::Result<usize> {
    occurrence
        .checked_sub(1)
        .and_then(|index| usize::try_from(index).ok())
        .ok_or_else(|| invalid("provider reference occurrence exceeds its dense namespace"))
}

impl NativeProviderProcess {
    pub fn call_provider_reference(
        &mut self,
        adapter: NativeAdapterId,
        arguments: &[NativeArgument<'_>],
    ) -> Result<NativeProviderReference, NativeProviderError> {
        let storage = self.installed_result_storage(adapter)?;
        if !matches!(storage, NativeResultStorage::SharedProviderReference { .. }) {
            return Err(invalid("native call has no admitted provider reference result").into());
        }
        let occurrence = self
            .provider_references
            .len()
            .checked_add(1)
            .and_then(|ordinal| u64::try_from(ordinal).ok())
            .ok_or_else(|| invalid("provider reference occurrence identities exhausted"))?;
        let packet = self.call_packet(adapter, arguments)?;
        let response = self.exchange(packet)?;
        let mut cursor = Cursor::new(&response);
        let reference = NativeProviderReference {
            era: self.era,
            adapter: cursor.number()?,
            occurrence: cursor.number()?,
        };
        cursor.finish()?;
        if reference.adapter != adapter.ordinal || reference.occurrence != occurrence {
            return Err(
                invalid("provider reference completion is stale or names another adapter").into(),
            );
        }
        self.provider_references.push(Some(ProviderReferenceResult {
            adapter: reference.adapter,
            storage,
        }));
        Ok(reference)
    }

    pub fn read_provider_reference(
        &mut self,
        reference: NativeProviderReference,
        offset: usize,
        output: &mut [u8],
    ) -> Result<(), NativeProviderError> {
        let NativeResultStorage::SharedProviderReference { byte_size, .. } =
            self.provider_reference_storage(reference)?
        else {
            return Err(invalid("provider reference has no admitted referent geometry").into());
        };
        if output.is_empty()
            || offset
                .checked_add(output.len())
                .is_none_or(|end| end > byte_size)
        {
            return Err(invalid("provider reference read exceeds its admitted referent").into());
        }
        let mut packet = Packet::new(8);
        packet.number(reference.adapter);
        packet.number(reference.occurrence);
        packet.number(offset as u64);
        packet.number(output.len() as u64);
        let response = self.exchange(packet)?;
        if response.len() != output.len() {
            return Err(invalid("provider reference read changed its exact extent").into());
        }
        output.copy_from_slice(&response);
        Ok(())
    }

    pub fn release_provider_reference(
        &mut self,
        reference: NativeProviderReference,
    ) -> Result<(), NativeProviderError> {
        self.provider_reference_storage(reference)?;
        let mut packet = Packet::new(9);
        packet.number(reference.adapter);
        packet.number(reference.occurrence);
        if !self.exchange(packet)?.is_empty() {
            return Err(
                invalid("provider reference release returned an unexpected payload").into(),
            );
        }
        self.provider_references[occurrence_index(reference.occurrence)?] = None;
        Ok(())
    }

    fn provider_reference_storage(
        &self,
        reference: NativeProviderReference,
    ) -> Result<NativeResultStorage, NativeProviderError> {
        if self.faulted || reference.era != self.era {
            return Err(invalid(
                "provider reference belongs to a faulted or different incarnation",
            )
            .into());
        }
        self.provider_references
            .get(occurrence_index(reference.occurrence)?)
            .and_then(Option::as_ref)
            .filter(|row| row.adapter == reference.adapter)
            .map(|row| row.storage)
            .ok_or_else(|| invalid("provider reference occurrence is absent or retired").into())
    }
}

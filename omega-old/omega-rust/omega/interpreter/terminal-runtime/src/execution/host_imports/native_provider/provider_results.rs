//! Live scalar leaves of one admitted provider result share one worker lease.
//! Registry retention is physical custody only; NativeImport authenticates the
//! safe contract before installing its returned-reference storage recipe.

use super::{StorageCodec, original_homes::Sponsor, rejection};
use native_provider_runtime::{NativeProviderError, NativeProviderReference};
use semantic_vocabulary::ScalarType;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use terminal_interpreter::{
    TerminalEffectRejection, TerminalEffectResult, TerminalReferencePayload,
    TerminalReferenceResult, TerminalReferenceScalarBacking, TerminalScalarBacking,
    TerminalScalarValue,
};

#[derive(Default)]
pub(super) struct LeaseRegistry {
    active: Cell<usize>,
    released: RefCell<Vec<NativeProviderReference>>,
}

impl LeaseRegistry {
    pub(super) fn retain(
        self: &Rc<Self>,
        reference: NativeProviderReference,
        sponsor: Rc<RefCell<Sponsor>>,
    ) -> Result<Rc<Lease>, NativeProviderError> {
        let active = self
            .active
            .get()
            .checked_add(1)
            .ok_or_else(|| NativeProviderError {
                reason: "native provider result lease count exceeds its namespace".into(),
                status: None,
            })?;
        self.active.set(active);
        Ok(Rc::new(Lease {
            registry: Rc::clone(self),
            reference,
            sponsor,
        }))
    }

    pub(super) fn active(&self) -> usize {
        self.active.get()
    }

    pub(super) fn take_released(&self) -> Vec<NativeProviderReference> {
        std::mem::take(&mut *self.released.borrow_mut())
    }
}

pub(super) struct Lease {
    registry: Rc<LeaseRegistry>,
    pub(super) reference: NativeProviderReference,
    sponsor: Rc<RefCell<Sponsor>>,
}

impl Drop for Lease {
    fn drop(&mut self) {
        // No sponsor borrow or IPC in Drop. A normal operation drains the
        // queue; this occurrence ceases to pin retirement immediately.
        self.registry.active.set(self.registry.active.get() - 1);
        self.registry.released.borrow_mut().push(self.reference);
    }
}

struct Scalar {
    lease: Rc<Lease>,
    offset: usize,
    stored: ScalarType,
    logical: ScalarType,
}

impl std::fmt::Debug for Scalar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeProviderScalar")
            .field("reference", &self.lease.reference)
            .field("offset", &self.offset)
            .finish_non_exhaustive()
    }
}

impl TerminalScalarBacking for Scalar {
    fn read(&self) -> Result<TerminalScalarValue, TerminalEffectRejection> {
        let size = super::super::scalar_codec::scalar_extent(self.stored)?;
        let mut bytes = [0; 16];
        self.lease.sponsor.borrow_mut().read_provider_reference(
            self.lease.reference,
            self.offset,
            &mut bytes[..size],
        )?;
        super::super::scalar_codec::decode(&bytes[..size], self.stored, self.logical)
    }

    fn write(&self, _value: TerminalScalarValue) -> Result<(), TerminalEffectRejection> {
        Err(rejection(
            "shared provider reference backing cannot be written",
        ))
    }
}

pub(super) fn complete(
    lease: Rc<Lease>,
    codec: &StorageCodec,
    scalar_order: &[usize],
) -> Result<TerminalEffectResult, TerminalEffectRejection> {
    let mut bytes = vec![0; codec.extent];
    lease
        .sponsor
        .borrow_mut()
        .read_provider_reference(lease.reference, 0, &mut bytes)?;
    let aggregate = codec.aggregate(&bytes)?;
    let scalars = scalar_order
        .iter()
        .map(|&index| &codec.fields[index])
        .map(|field| TerminalReferenceScalarBacking {
            path: field.path.clone(),
            backing: Rc::new(Scalar {
                lease: Rc::clone(&lease),
                offset: field.offset,
                stored: field.stored,
                logical: field.logical,
            }) as Rc<dyn TerminalScalarBacking>,
        })
        .collect::<Vec<_>>();
    Ok(TerminalEffectResult::Reference(TerminalReferenceResult {
        referent: TerminalReferencePayload::Resident { aggregate, scalars },
        references: Vec::new(),
    }))
}

//! Activation-local primitive roots share backing only through explicit loans.

use std::collections::BTreeSet;

use semantic_vocabulary::{PlaceId, ScalarType, StructuralPlaceKind, StructuralTypeId, ValueId};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralMultiplicity,
    TerminalModule,
};

use super::{
    ExecutableMachine, StructuralRuntimePlace, TerminalExecution, TerminalInterpretError,
    TerminalStructuralValue, terminal_scalar_belongs_to_type,
};

pub(super) struct PrimitiveLocalIdentities {
    reserved: BTreeSet<u64>,
    next: Option<u64>,
}

impl PrimitiveLocalIdentities {
    #[cfg(test)]
    pub(crate) fn with_reserved_identities(reserved: impl IntoIterator<Item = u64>) -> Self {
        Self {
            reserved: reserved.into_iter().collect(),
            next: Some(0),
        }
    }

    pub(super) fn new(module: &TerminalModule, arguments: &[TerminalStructuralValue]) -> Self {
        Self {
            reserved: module
                .machines
                .iter()
                .flat_map(|machine| &machine.structural_places)
                .map(|place| place.id.get())
                .chain(arguments.iter().map(|value| value.opaque_identity))
                .collect(),
            next: Some(0),
        }
    }

    fn allocate(&mut self) -> Result<u64, TerminalInterpretError> {
        loop {
            let identity = self
                .next
                .ok_or(TerminalInterpretError::StructuralIdentityExhausted)?;
            self.next = identity.checked_add(1);
            if !self.reserved.contains(&identity) {
                return Ok(identity);
            }
        }
    }

    pub(super) fn reserve_host(
        &mut self,
        value: &TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        let identity = value.opaque_identity;
        // A host result cannot claim an identity allocated to an interpreter local.
        if self.next.is_none_or(|next| identity < next) && !self.reserved.contains(&identity) {
            return Err(TerminalInterpretError::StructuralArgumentAliasing(identity));
        }
        self.reserved.insert(identity);
        Ok(())
    }
}

fn local_type(machine: &ExecutableMachine, place: PlaceId) -> Option<StructuralTypeId> {
    let declaration = machine
        .structural_places
        .iter()
        .find(|entry| entry.id == place)?;
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = declaration.kind
    else {
        return None;
    };
    let operation = machine
        .blocks
        .values()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == producer)?;
    let result = operation.result.structural()?;
    (matches!(
        operation.kind,
        OperationKind::EstablishPrimitiveLocal { .. }
    ) && result.place == place
        && result.structural_type == structural_type
        && result.multiplicity == StructuralMultiplicity::Unrestricted
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty())
    .then_some(structural_type)
}

impl TerminalExecution {
    fn primitive_access(
        &self,
        place: PlaceId,
        writing: bool,
    ) -> Result<(StructuralRuntimePlace, ScalarType), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let machine = self
            .machines
            .get(&self.current_machine)
            .ok_or_else(invalid)?;
        let structural_type = local_type(machine, place)
            .or_else(|| {
                machine
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == place)
                    .filter(|parameter| {
                        (if writing {
                            matches!(
                                parameter.access,
                                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                            )
                        } else {
                            matches!(
                                parameter.access,
                                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                            )
                        }) && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                            && parameter.qualifications.is_empty()
                            && parameter.projected_qualifications.is_empty()
                    })
                    .map(|parameter| parameter.structural_type)
            })
            .ok_or_else(invalid)?;
        let Some(terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar_type)) = self
            .structural_types
            .get(&structural_type)
            .map(|entry| &entry.shape)
        else {
            return Err(invalid());
        };
        let view = self.structural_values.get(&place).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(place),
        )?;
        if view.structural_type != structural_type {
            return Err(invalid());
        }
        // Initialized array backing has one mutation owner. Projected scalar
        // storage is not a second, potentially stale copy of its elements.
        if self.structural_byte_arrays.keys().any(|array| {
            array.opaque_identity == view.opaque_identity && view.path.starts_with(&array.path)
        }) {
            return Err(invalid());
        }
        Ok((StructuralRuntimePlace::from(view), *scalar_type))
    }

    pub(super) fn execute_primitive_establishment(
        &mut self,
        operation: &Operation,
        value: ValueId,
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let result = operation.result.structural().ok_or_else(invalid)?;
        let machine = self
            .machines
            .get(&self.current_machine)
            .ok_or_else(invalid)?;
        if local_type(machine, result.place) != Some(result.structural_type)
            || !machine.structural_places.iter().any(|place| {
                place.id == result.place && matches!(place.kind,
                    StructuralPlaceKind::OperationResult { producer, .. } if producer == operation.id)
            })
        {
            return Err(invalid());
        }
        let Some(terminal_psi::StructuralTypeShape::PrimitiveScalar(expected)) = self
            .structural_types
            .get(&result.structural_type)
            .map(|entry| &entry.shape)
        else {
            return Err(invalid());
        };
        let scalar = self
            .values
            .get(&value)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(value))?;
        if scalar.scalar_type() != *expected || !terminal_scalar_belongs_to_type(scalar) {
            return Err(invalid());
        }
        let identity = self.primitive_local_identities.allocate()?;
        let view = TerminalStructuralValue {
            opaque_identity: identity,
            structural_type: result.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        };
        // A reentered definition replaces this activation's old local only.
        // Local results cannot escape through owned arguments or structural returns.
        if let Some(previous) = self.structural_values.insert(result.place, view.clone()) {
            self.structural_primitive_storage
                .remove(&StructuralRuntimePlace::from(&previous));
        }
        self.structural_primitive_storage
            .insert(StructuralRuntimePlace::from(&view), scalar);
        Ok(())
    }

    pub(super) fn execute_primitive_read(
        &mut self,
        operation: &Operation,
        source: PlaceId,
    ) -> Result<(), TerminalInterpretError> {
        let result = operation
            .result
            .scalar()
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let (storage, scalar_type) = self.primitive_access(source, false)?;
        let scalar = self
            .structural_primitive_storage
            .get(&storage)
            .copied()
            .ok_or(TerminalInterpretError::StructuralPrimitiveStorageMissing(
                source,
            ))?;
        if result.scalar_type != scalar_type
            || scalar.scalar_type() != scalar_type
            || !terminal_scalar_belongs_to_type(scalar)
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        self.values.insert(result.id, scalar);
        Ok(())
    }

    pub(super) fn execute_primitive_store(
        &mut self,
        operation: &Operation,
        destination: PlaceId,
        value: ValueId,
    ) -> Result<(), TerminalInterpretError> {
        if operation.result != OperationResult::Unit {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let (storage, scalar_type) = self.primitive_access(destination, true)?;
        let scalar = self
            .values
            .get(&value)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(value))?;
        if scalar.scalar_type() != scalar_type || !terminal_scalar_belongs_to_type(scalar) {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let stored = self.structural_primitive_storage.get_mut(&storage).ok_or(
            TerminalInterpretError::StructuralPrimitiveStorageMissing(destination),
        )?;
        *stored = scalar;
        Ok(())
    }

    pub(super) fn primitive_local_count(&self) -> usize {
        self.machines
            .get(&self.current_machine)
            .map_or(0, |machine| {
                self.structural_values
                    .keys()
                    .filter(|place| local_type(machine, **place).is_some())
                    .count()
            })
    }

    pub(super) fn retire_primitive_locals(&mut self) {
        let Some(machine) = self.machines.get(&self.current_machine) else {
            return;
        };
        for place in &machine.structural_places {
            if local_type(machine, place.id).is_some()
                && let Some(view) = self.structural_values.remove(&place.id)
            {
                self.structural_primitive_storage
                    .remove(&StructuralRuntimePlace::from(&view));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_identities_skip_reserved_values_and_do_not_wrap() {
        let mut identities = PrimitiveLocalIdentities {
            reserved: BTreeSet::from([0, 2]),
            next: Some(0),
        };
        assert_eq!(identities.allocate(), Ok(1));
        assert_eq!(identities.allocate(), Ok(3));
        identities.next = Some(u64::MAX);
        assert_eq!(identities.allocate(), Ok(u64::MAX));
        assert_eq!(
            identities.allocate(),
            Err(TerminalInterpretError::StructuralIdentityExhausted)
        );
    }

    #[test]
    fn host_results_cannot_reintroduce_allocated_local_identity() {
        let mut identities = PrimitiveLocalIdentities {
            reserved: BTreeSet::from([0]),
            next: Some(0),
        };
        let view = |opaque_identity| TerminalStructuralValue {
            opaque_identity,
            structural_type: StructuralTypeId::new(1).unwrap(),
            qualifications: Vec::new(),
            path: Vec::new(),
        };
        identities.reserve_host(&view(2)).unwrap();
        assert_eq!(identities.allocate(), Ok(1));
        assert_eq!(
            identities.reserve_host(&view(1)),
            Err(TerminalInterpretError::StructuralArgumentAliasing(1))
        );
        identities.reserve_host(&view(0)).unwrap();
        assert_eq!(identities.allocate(), Ok(3));
        assert_eq!(
            identities.reserved,
            BTreeSet::from([0, 2]),
            "allocation does not retain one tombstone per dead local"
        );
    }
}

//! Geometry of one caller namespace, captured independently of value liveness.
//!
//! Operand preparation can move producers into private blocks or select a
//! captured referent other than the reference descriptor. Crash substitution
//! asks this same publication owner about that physical root; a callee's
//! expected type cannot reconstruct a missing caller declaration.
use super::{LoweringError, PlaceId, StructuralTypeId};
use crate::emission::operation_emission::buffer::OperationBuffer;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PublishedStructuralTypes {
    homes: Vec<(PlaceId, StructuralTypeId)>,
}

impl PublishedStructuralTypes {
    pub(super) fn from_rows(
        rows: impl IntoIterator<Item = (PlaceId, StructuralTypeId)>,
    ) -> Result<Self, LoweringError> {
        let mut homes = rows.into_iter().collect::<Vec<_>>();
        homes.sort_unstable_by_key(|(place, _)| *place);
        for pair in homes.windows(2) {
            if pair[0].0 == pair[1].0 && pair[0].1 != pair[1].1 {
                return Err(LoweringError::ConflictingStructuralPlaceType {
                    place: pair[0].0,
                    first: pair[0].1,
                    second: pair[1].1,
                });
            }
        }
        homes.dedup();
        Ok(Self { homes })
    }

    pub(super) fn homes(&self) -> impl Iterator<Item = (PlaceId, StructuralTypeId)> + '_ {
        self.homes.iter().copied()
    }

    pub(crate) fn get(&self, place: PlaceId) -> Option<StructuralTypeId> {
        self.homes
            .binary_search_by_key(&place, |(place, _)| *place)
            .ok()
            .map(|index| self.homes[index].1)
    }

    pub(crate) fn root_type(
        &self,
        argument: &terminal_psi::StructuralArgument,
    ) -> Result<StructuralTypeId, LoweringError> {
        self.get(argument.place)
            .ok_or_else(|| LoweringError::MissingCrashArgumentSource {
                argument: argument.clone(),
            })
    }

    fn retain(&mut self, place: PlaceId, carrier: StructuralTypeId) -> Result<(), LoweringError> {
        match self.homes.binary_search_by_key(&place, |(place, _)| *place) {
            Ok(index) if self.homes[index].1 != carrier => {
                Err(LoweringError::ConflictingStructuralPlaceType {
                    place,
                    first: self.homes[index].1,
                    second: carrier,
                })
            }
            Ok(_) => Ok(()),
            Err(index) => {
                self.homes.insert(index, (place, carrier));
                Ok(())
            }
        }
    }

    /// Complete anonymous operands retain their actual producer geometry,
    /// including operations sealed out of the current straight-line buffer.
    /// Projected aliases describe a leaf, so they cannot declare its root.
    pub(crate) fn with_completed(
        mut self,
        operations: &OperationBuffer,
    ) -> Result<Self, LoweringError> {
        for result in operations
            .operations
            .iter()
            .filter_map(|operation| operation.result.structural())
        {
            self.retain(result.place, result.structural_type)?;
        }
        for (_, binding) in &operations.structural_values {
            if binding.path().is_empty() {
                self.retain(binding.place(), binding.structural_type())?;
            }
        }
        for (place, carrier) in &operations.literal_types {
            self.retain(*place, *carrier)?;
        }
        Ok(self)
    }
}

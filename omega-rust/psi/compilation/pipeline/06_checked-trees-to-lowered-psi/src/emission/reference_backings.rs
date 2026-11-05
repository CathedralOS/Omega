//! Keep the evaluated source of a reference beside the value that carries it.
//! A descriptor operation and a borrowed block arrival have the same obligation:
//! later state transfers must retain that source, without replaying its selectors.

use crate::shared::lowering_error::{LoweringError, unsupported};
use semantic_vocabulary::PlaceId;
use terminal_psi::StructuralArgument;

pub(crate) struct ReferenceBackings {
    first_place: u64,
    sources: Vec<Vec<StructuralArgument>>,
}

impl ReferenceBackings {
    pub(crate) fn new(first_place: u64) -> Self {
        Self {
            first_place,
            sources: Vec::new(),
        }
    }

    fn index(&self, place: PlaceId) -> Result<usize, LoweringError> {
        place
            .get()
            .checked_sub(self.first_place)
            .and_then(|index| usize::try_from(index).ok())
            .ok_or(LoweringError::Unsupported(
                "reference value belongs outside its emission frame",
            ))
    }

    pub(crate) fn record(
        &mut self,
        place: PlaceId,
        source: StructuralArgument,
    ) -> Result<(), LoweringError> {
        let index = self.index(place)?;
        if self.sources.len() <= index {
            self.sources.resize_with(index + 1, Vec::new);
        }
        self.sources[index].push(source);
        Ok(())
    }

    pub(crate) fn source(&self, place: PlaceId) -> Result<&StructuralArgument, LoweringError> {
        let sources = self
            .sources
            .get(self.index(place)?)
            .ok_or(LoweringError::Unsupported(
                "completed reference has no recorded backing",
            ))?;
        let Some(first) = sources.first() else {
            return unsupported("completed reference has no recorded backing");
        };
        if sources.iter().any(|source| source != first) {
            return unsupported("completed reference has several distinct backing alternatives");
        }
        Ok(first)
    }
}

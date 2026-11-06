//! Keep the evaluated source of a reference beside the value that carries it.
//! A descriptor operation and a borrowed block arrival have the same obligation:
//! later state transfers must retain that source, without replaying its selectors.
//! Missing associations stay unavailable; the state transfer owns their refusal.

use semantic_vocabulary::PlaceId;
use terminal_psi::StructuralArgument;

pub(crate) struct ReferenceBackings {
    first_place: u64,
    sources: Vec<Option<ReferenceBacking>>,
    valid_frame: bool,
}

#[derive(Clone)]
enum ReferenceBacking {
    Source(StructuralArgument),
    Unavailable,
}

impl ReferenceBackings {
    pub(crate) fn new(first_place: u64) -> Self {
        Self {
            first_place,
            sources: Vec::new(),
            valid_frame: true,
        }
    }

    fn index(&self, place: PlaceId) -> Option<usize> {
        place
            .get()
            .checked_sub(self.first_place)
            .and_then(|index| usize::try_from(index).ok())
    }

    pub(crate) fn record(&mut self, place: PlaceId, source: StructuralArgument) {
        self.merge(place, ReferenceBacking::Source(source));
    }

    pub(crate) fn source(&self, place: PlaceId) -> Option<&StructuralArgument> {
        if !self.valid_frame {
            return None;
        }
        match self
            .sources
            .get(self.index(place)?)
            .and_then(Option::as_ref)?
        {
            ReferenceBacking::Source(source) => Some(source),
            ReferenceBacking::Unavailable => None,
        }
    }

    /// Moving a whole descriptor through a value join retains its backing.
    /// A projection below that descriptor denotes another value and needs its
    /// own formation; the enclosing descriptor cannot supply that association.
    /// An unresolved arm remains unresolved even when another arm has a source.
    pub(crate) fn forward(&mut self, argument: &StructuralArgument, destination: PlaceId) {
        if argument.path.is_empty() && argument.place == destination {
            return;
        }
        let source = if argument.path.is_empty() {
            argument
                .place
                .get()
                .checked_sub(self.first_place)
                .and_then(|index| usize::try_from(index).ok())
                .and_then(|index| self.sources.get(index))
                .and_then(Option::as_ref)
                .cloned()
                .unwrap_or(ReferenceBacking::Unavailable)
        } else {
            ReferenceBacking::Unavailable
        };
        self.merge(destination, source);
    }

    fn merge(&mut self, place: PlaceId, source: ReferenceBacking) {
        let Some(index) = self.index(place) else {
            // An out-of-frame producer invalidates this association table.
            // The state transfer reports any required missing source.
            self.valid_frame = false;
            return;
        };
        if self.sources.len() <= index {
            self.sources.resize_with(index + 1, || None);
        }
        let slot = &mut self.sources[index];
        match (slot.as_ref(), &source) {
            (None, _) => *slot = Some(source),
            (Some(ReferenceBacking::Source(left)), ReferenceBacking::Source(right))
                if left == right => {}
            _ => *slot = Some(ReferenceBacking::Unavailable),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_psi::StructuralAccess;

    fn argument(place: u64) -> StructuralArgument {
        StructuralArgument {
            place: PlaceId::new(place).unwrap(),
            path: Vec::new(),
            access: StructuralAccess::MutableBorrow,
        }
    }

    #[test]
    fn owned_descriptor_join_keeps_its_original_backing() {
        let mut backings = ReferenceBackings::new(10);
        let source = argument(2);
        backings.record(argument(10).place, source.clone());
        backings.forward(&argument(10), argument(11).place);
        assert_eq!(backings.source(argument(11).place), Some(&source));
    }

    #[test]
    fn an_out_of_frame_producer_cannot_supply_another_local_value() {
        let mut backings = ReferenceBackings::new(10);
        backings.record(argument(5).place, argument(2));
        backings.record(argument(10).place, argument(3));
        assert!(backings.source(argument(5).place).is_none());
        assert!(backings.source(argument(10).place).is_none());
    }

    #[test]
    fn one_known_arm_cannot_supply_an_unresolved_or_different_arm() {
        for unknown in [true, false] {
            for reverse in [true, false] {
                let mut backings = ReferenceBackings::new(10);
                backings.record(argument(10).place, argument(2));
                if !unknown {
                    backings.record(argument(11).place, argument(3));
                }
                let arms = if reverse { [11, 10] } else { [10, 11] };
                for arm in arms {
                    backings.forward(&argument(arm), argument(12).place);
                }
                assert!(backings.source(argument(12).place).is_none());
            }
        }
    }
}

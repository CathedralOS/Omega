//! A packed payload has one owner even after its frame binding retires.
//! Retained loans address backing; ordinary operands address live bindings.
use super::TerminalScalarArrayValue;
use crate::errors::{TerminalInterpretError, malformed};
use semantic_vocabulary::PlaceId;
use std::collections::BTreeMap;

/// Live packed bindings and payload retained by captured frame-home loans.
/// Ordinary operand lookup sees only live bindings. A captured storage endpoint
/// can still reach a retired payload until the storage collector proves that no
/// resident or outgoing descriptor retains its exact activation home.
#[derive(Default)]
pub(crate) struct ScalarArrayStorage {
    live: BTreeMap<PlaceId, TerminalScalarArrayValue>,
    retired: BTreeMap<PlaceId, TerminalScalarArrayValue>,
}

impl From<BTreeMap<PlaceId, TerminalScalarArrayValue>> for ScalarArrayStorage {
    fn from(live: BTreeMap<PlaceId, TerminalScalarArrayValue>) -> Self {
        Self {
            live,
            retired: BTreeMap::new(),
        }
    }
}

impl ScalarArrayStorage {
    pub(crate) fn get(&self, place: &PlaceId) -> Option<&TerminalScalarArrayValue> {
        self.live.get(place)
    }

    pub(crate) fn contains_key(&self, place: &PlaceId) -> bool {
        self.live.contains_key(place)
    }

    pub(crate) fn keys(&self) -> impl Iterator<Item = &PlaceId> {
        self.live.keys()
    }

    /// Ending a binding never disposes payload retained by a captured loan.
    pub(crate) fn remove(&mut self, place: &PlaceId) {
        self.retire_packed_home(place);
    }

    pub(crate) fn publish(
        &mut self,
        place: PlaceId,
        value: TerminalScalarArrayValue,
    ) -> Result<(), TerminalInterpretError> {
        // A captured frame home has no rebinding epoch. Reusing its name while
        // its previous payload is retained would silently retarget that loan.
        if self.live.contains_key(&place) || self.retired.contains_key(&place) {
            return Err(malformed());
        }
        self.live.insert(place, value);
        Ok(())
    }

    pub(crate) fn backing(&self, place: &PlaceId) -> Option<&TerminalScalarArrayValue> {
        self.live.get(place).or_else(|| self.retired.get(place))
    }

    pub(crate) fn backing_mut(&mut self, place: &PlaceId) -> Option<&mut TerminalScalarArrayValue> {
        if self.live.contains_key(place) {
            self.live.get_mut(place)
        } else {
            self.retired.get_mut(place)
        }
    }

    pub(crate) fn has_retired(&self) -> bool {
        !self.retired.is_empty()
    }

    pub(crate) fn retains_retired(&self, place: &PlaceId) -> bool {
        self.retired.contains_key(place)
    }

    pub(crate) fn collect_retired(&mut self, mut retained: impl FnMut(PlaceId) -> bool) {
        self.retired.retain(|place, _| retained(*place));
    }
}

/// The ordered cleanup transaction has two inventories: a key-only preflight
/// and the live payload owner. Both retire exactly one available packed name.
pub(crate) trait PackedHomeRetirement {
    fn contains_packed_home(&self, place: &PlaceId) -> bool;
    fn retire_packed_home(&mut self, place: &PlaceId);
}

impl PackedHomeRetirement for ScalarArrayStorage {
    fn contains_packed_home(&self, place: &PlaceId) -> bool {
        self.live.contains_key(place)
    }

    fn retire_packed_home(&mut self, place: &PlaceId) {
        if let Some(value) = self.live.remove(place) {
            self.retired.insert(*place, value);
        }
    }
}

impl PackedHomeRetirement for std::collections::BTreeSet<PlaceId> {
    fn contains_packed_home(&self, place: &PlaceId) -> bool {
        self.contains(place)
    }

    fn retire_packed_home(&mut self, place: &PlaceId) {
        self.remove(place);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::values::TerminalScalarValue;
    use semantic_vocabulary::StructuralTypeId;

    fn payload() -> TerminalScalarArrayValue {
        TerminalScalarArrayValue {
            storage_identity: None,
            qualifications: Vec::new(),
            structural_type: StructuralTypeId::new(1).unwrap(),
            elements: vec![TerminalScalarValue::Boolean(true)],
        }
    }

    #[test]
    fn binding_retirement_preserves_captured_payload_without_copying_or_rebinding() {
        let place = PlaceId::new(1).unwrap();
        let mut arrays = ScalarArrayStorage::default();
        arrays.publish(place, payload()).unwrap();
        let original = arrays.get(&place).unwrap().elements.as_ptr();
        arrays.remove(&place);
        assert!(arrays.get(&place).is_none());
        assert_eq!(arrays.backing(&place).unwrap().elements.as_ptr(), original);
        assert!(arrays.publish(place, payload()).is_err());
        arrays.collect_retired(|retained| retained == place);
        assert!(arrays.backing(&place).is_some());
        arrays.collect_retired(|_| false);
        assert!(arrays.backing(&place).is_none());
        arrays.publish(place, payload()).unwrap();
    }
}

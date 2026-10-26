//! Packed bindings and payload retained by captured frame-home loans.
//!
//! Ordinary operands see only live homes; frozen endpoints may still reach a
//! retired payload. One funded row owns both states, so cleanup retirement
//! transfers no cells and cannot allocate after its custody preflight.

use super::TerminalScalarArrayValue;
use crate::errors::{TerminalInterpretError, malformed};
use crate::runtime_rows::RuntimeRows;
use evaluation_accounting::EvaluationInvocationAccount;
use semantic_vocabulary::PlaceId;

struct PackedBinding {
    live: bool,
    value: TerminalScalarArrayValue,
}

pub(crate) struct ScalarArrayBindings {
    rows: RuntimeRows<PlaceId, PackedBinding>,
    live_count: usize,
}

impl ScalarArrayBindings {
    pub(crate) fn new(account: Option<&EvaluationInvocationAccount>) -> Self {
        Self {
            rows: RuntimeRows::new(account),
            live_count: 0,
        }
    }

    pub(crate) fn take(&mut self) -> Self {
        Self {
            rows: self.rows.take(),
            live_count: std::mem::replace(&mut self.live_count, 0),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.live_count
    }

    pub(crate) fn get(&self, place: &PlaceId) -> Option<&TerminalScalarArrayValue> {
        self.rows
            .get(place)
            .filter(|binding| binding.live)
            .map(|binding| &binding.value)
    }

    pub(crate) fn get_mut(&mut self, place: &PlaceId) -> Option<&mut TerminalScalarArrayValue> {
        self.rows
            .get_mut(place)
            .filter(|binding| binding.live)
            .map(|binding| &mut binding.value)
    }

    pub(crate) fn contains_key(&self, place: &PlaceId) -> bool {
        self.get(place).is_some()
    }

    pub(crate) fn keys(&self) -> impl Iterator<Item = &PlaceId> {
        self.rows
            .iter()
            .filter(|(_, binding)| binding.live)
            .map(|(place, _)| place)
    }

    /// Ending a binding does not dispose payload retained by a captured loan.
    pub(crate) fn remove(&mut self, place: &PlaceId) {
        self.retire_packed_home(place);
    }

    pub(crate) fn publish(
        &mut self,
        place: PlaceId,
        value: TerminalScalarArrayValue,
    ) -> Result<(), TerminalInterpretError> {
        // Captured frame homes have no rebinding epoch. Reusing a retained
        // name would silently redirect a loan, even if its live binding ended.
        let live_count = self.live_count.checked_add(1).ok_or_else(malformed)?;
        self.rows
            .insert_vacant(place, PackedBinding { live: true, value })?;
        self.live_count = live_count;
        Ok(())
    }

    pub(crate) fn backing(&self, place: &PlaceId) -> Option<&TerminalScalarArrayValue> {
        self.rows.get(place).map(|binding| &binding.value)
    }

    pub(crate) fn backing_mut(&mut self, place: &PlaceId) -> Option<&mut TerminalScalarArrayValue> {
        self.rows.get_mut(place).map(|binding| &mut binding.value)
    }

    pub(crate) fn has_retired(&self) -> bool {
        self.rows.len() != self.live_count
    }

    pub(crate) fn retains_retired(&self, place: &PlaceId) -> bool {
        self.rows.get(place).is_some_and(|binding| !binding.live)
    }

    pub(crate) fn collect_retired(&mut self, mut retained: impl FnMut(PlaceId) -> bool) {
        self.rows
            .retain(|place, binding| binding.live || retained(*place));
    }
}

/// The cleanup transaction preflights keys, then retires the actual payload home.
/// Both owners expose one live binding; neither consults a descriptor as authority.
pub(crate) trait PackedHomeRetirement {
    fn contains_packed_home(&self, place: &PlaceId) -> bool;
    fn retire_packed_home(&mut self, place: &PlaceId);
}

impl PackedHomeRetirement for ScalarArrayBindings {
    fn contains_packed_home(&self, place: &PlaceId) -> bool {
        self.contains_key(place)
    }

    fn retire_packed_home(&mut self, place: &PlaceId) {
        if let Some(binding) = self.rows.get_mut(place)
            && binding.live
        {
            binding.live = false;
            self.live_count -= 1;
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
        TerminalScalarArrayValue::collect(
            StructuralTypeId::new(1).unwrap(),
            Vec::new(),
            1,
            [Ok(TerminalScalarValue::Boolean(true))],
            None,
        )
        .unwrap()
    }

    #[test]
    fn binding_retirement_preserves_captured_payload_without_copying_or_rebinding() {
        let place = PlaceId::new(1).unwrap();
        let mut arrays = ScalarArrayBindings::new(None);
        arrays.publish(place, payload()).unwrap();
        let original = arrays.get(&place).unwrap().elements().as_ptr();
        arrays.remove(&place);
        assert!(arrays.get(&place).is_none());
        assert_eq!(
            arrays.backing(&place).unwrap().elements().as_ptr(),
            original
        );
        assert!(arrays.publish(place, payload()).is_err());
        arrays.collect_retired(|retained| retained == place);
        assert!(arrays.backing(&place).is_some());
        arrays.collect_retired(|_| false);
        assert!(arrays.backing(&place).is_none());
        arrays.publish(place, payload()).unwrap();
    }

    #[test]
    fn exact_capacity_retirement_keeps_funding_until_the_last_home_is_released() {
        use evaluation_accounting::{
            BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
            TerminalStorageScheduleIdentity,
        };
        let sponsor = BuildEvaluationSponsor::new_with_measurement(
            BuildEvaluationSponsorLimits::new(100, 8, 2, 2, 2, 100, 100, 100).unwrap(),
            EvaluationMeasurementIdentity::TerminalPsi {
                fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
                storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
            },
        )
        .unwrap();
        let account = sponsor
            .begin_invocation(sponsor.measurement(), 100)
            .unwrap();
        let place = PlaceId::new(1).unwrap();
        let other = PlaceId::new(2).unwrap();
        let value = TerminalScalarArrayValue::collect(
            StructuralTypeId::new(1).unwrap(),
            Vec::new(),
            1,
            [Ok(TerminalScalarValue::Boolean(true))],
            Some(&account),
        )
        .unwrap();
        let mut arrays = ScalarArrayBindings::new(Some(&account));
        arrays.publish(place, value).unwrap();
        let original = arrays.get(&place).unwrap().elements().as_ptr();
        assert_eq!(
            sponsor.live_storage_cells(),
            2,
            "one payload cell and one home row"
        );
        arrays.retire_packed_home(&place);
        assert_eq!(
            sponsor.live_storage_cells(),
            2,
            "retirement must not reserve another row"
        );
        assert!(arrays.get(&place).is_none());
        assert_eq!(
            arrays.backing(&place).unwrap().elements().as_ptr(),
            original
        );
        let alias = arrays.backing(&place).unwrap().clone();
        assert!(
            arrays.publish(place, alias).is_err(),
            "a captured home cannot be rebound"
        );
        arrays.collect_retired(|retained| retained == place);
        assert_eq!(
            sponsor.live_storage_cells(),
            2,
            "the live loan retains its funded payload"
        );
        let mut saved = arrays.take();
        let alias = saved.backing(&place).unwrap().clone();
        assert!(
            arrays.publish(other, alias).is_err(),
            "the empty next frame keeps its account"
        );
        assert_eq!(sponsor.live_storage_cells(), 2);
        saved.collect_retired(|_| false);
        assert!(saved.backing(&place).is_none());
        assert_eq!(
            sponsor.live_storage_cells(),
            0,
            "GC retires both row and payload leases"
        );
        drop(saved);
        drop(arrays);
        assert_eq!(sponsor.peak_live_cells(), 2);
    }
}

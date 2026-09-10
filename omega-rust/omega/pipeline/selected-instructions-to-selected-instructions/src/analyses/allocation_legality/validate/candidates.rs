//! Replay-owned general candidates for one immutable function and register environment.
//!
//! Register-order replay revisits a location for every live value. Keep only visited
//! class/location rows for this invocation; no row survives into another function or
//! admission. Derivation remains independent of the producer. Fixed-view checks stay
//! at each original register occurrence, including views excluded from general
//! allocation, so preparation cannot change their error order or authority.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use register_model::{
    RegisterClass, RegisterClassId, RegisterUnitId, RegisterViewId, ValidatedPhysicalRegisterModel,
    ValidatedRegisterReservationProfile,
};
use selected_instructions::SelectedBlockId;

use crate::{FunctionLiveRanges, LiveRangePoint};

#[derive(Clone)]
pub(super) struct GeneralCandidates {
    pub(super) occupied: BTreeSet<RegisterUnitId>,
    pub(super) views: Vec<RegisterViewId>,
}

pub(super) struct ReplayCandidates<'input> {
    function: &'input FunctionLiveRanges,
    physical: &'input ValidatedPhysicalRegisterModel,
    reservations: &'input ValidatedRegisterReservationProfile,
    rows: BTreeMap<(RegisterClassId, SelectedBlockId, LiveRangePoint), GeneralCandidates>,
    #[cfg(test)]
    uncached: bool,
}

impl<'input> ReplayCandidates<'input> {
    pub(super) fn new(
        function: &'input FunctionLiveRanges,
        physical: &'input ValidatedPhysicalRegisterModel,
        reservations: &'input ValidatedRegisterReservationProfile,
    ) -> Self {
        Self {
            function,
            physical,
            reservations,
            rows: BTreeMap::new(),
            #[cfg(test)]
            uncached: false,
        }
    }

    #[cfg(test)]
    pub(super) fn uncached(
        function: &'input FunctionLiveRanges,
        physical: &'input ValidatedPhysicalRegisterModel,
        reservations: &'input ValidatedRegisterReservationProfile,
    ) -> Self {
        Self {
            uncached: true,
            ..Self::new(function, physical, reservations)
        }
    }

    // Class and availability are checked in original register order by the caller,
    // against the same immutable availability input throughout this function.
    pub(super) fn at(
        &mut self,
        class: &RegisterClass,
        available: &[RegisterViewId],
        block: SelectedBlockId,
        point: LiveRangePoint,
    ) -> Cow<'_, GeneralCandidates> {
        let derive = || {
            derive(
                self.function,
                self.physical,
                self.reservations,
                class,
                available,
                block,
                point,
            )
        };
        // The test oracle executes the original occupancy/view scan at every
        // occurrence; it never reads or populates retained candidate rows.
        #[cfg(test)]
        if self.uncached {
            return Cow::Owned(derive());
        }
        Cow::Borrowed(
            self.rows
                .entry((class.id, block, point))
                .or_insert_with(derive),
        )
    }
}

fn derive(
    function: &FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    reservations: &ValidatedRegisterReservationProfile,
    class: &RegisterClass,
    available: &[RegisterViewId],
    block: SelectedBlockId,
    point: LiveRangePoint,
) -> GeneralCandidates {
    let mut occupied = reservations
        .reserved_units()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for row in &function.architectural_units {
        if row.fragments.iter().any(|fragment| {
            fragment.block == block && fragment.start <= point && point < fragment.end
        }) || row
            .actions
            .iter()
            .any(|action| action.block == block && action.point == point)
        {
            occupied.insert(row.unit);
        }
    }
    let views = class
        .views
        .iter()
        .filter(|view_id| available.binary_search(view_id).is_ok())
        .filter_map(|view_id| {
            let view = physical
                .model()
                .views
                .iter()
                .find(|view| view.id == *view_id)?;
            (view.allocatable
                && view
                    .units
                    .iter()
                    .chain(&view.write_units)
                    .all(|unit| !occupied.contains(unit)))
            .then_some(*view_id)
        })
        .collect();
    GeneralCandidates { occupied, views }
}

//! Physical-view availability, reservation, and fixed-view filtering.

use std::collections::BTreeMap;

use register_model::{
    RegisterClass, RegisterClassId, RegisterView, RegisterViewId, ValidatedPhysicalRegisterModel,
    ValidatedRegisterReservationProfile,
};
use selected_instructions::SelectedBlockId;

use crate::AllocationLegalityError;
use selected_instructions::{FunctionLiveRanges, LiveRangePoint, VirtualLiveRange};

/// General candidates depend on the block, class, and architectural availability,
/// not the value visiting it. Availability is constant between the prepared unit
/// fragment/action boundaries, so cache requested cells rather than individual
/// points. Register starts and fixed-constraint boundaries inside one cell would
/// otherwise repeat the same unit-conflict walks and retain duplicate vectors.
/// Keep this sparse table local to the immutable function/environment; a spill
/// changes positions and requires a new table. Fixed constraints still run at
/// each register's actual point, outside the reusable unconstrained result.
pub(super) struct CandidateViews<'a> {
    function: &'a FunctionLiveRanges,
    physical: &'a ValidatedPhysicalRegisterModel,
    reservations: &'a ValidatedRegisterReservationProfile,
    candidates: BTreeMap<(RegisterClassId, SelectedBlockId, usize), Vec<RegisterViewId>>,
    boundaries: Vec<LiveRangePoint>,
}

impl<'a> CandidateViews<'a> {
    pub(super) fn new(
        function: &'a FunctionLiveRanges,
        physical: &'a ValidatedPhysicalRegisterModel,
        reservations: &'a ValidatedRegisterReservationProfile,
    ) -> Self {
        // Candidate availability can change only at a unit fragment edge or
        // on either side of a one-phase architectural action. Points are
        // function-global; unrelated block boundaries merely subdivide work.
        let mut boundaries = Vec::new();
        for unit in &function.architectural_units {
            for fragment in &unit.fragments {
                boundaries.extend([fragment.start, fragment.end]);
            }
            for action in &unit.actions {
                boundaries.push(action.point);
                if let Some(end) = action.point.0.checked_add(1) {
                    boundaries.push(LiveRangePoint(end));
                }
            }
        }
        boundaries.sort_unstable();
        boundaries.dedup();
        Self {
            function,
            physical,
            reservations,
            candidates: BTreeMap::new(),
            boundaries,
        }
    }

    pub(super) fn boundaries(&self) -> &[LiveRangePoint] {
        &self.boundaries
    }

    // The function caller validates one immutable availability input before
    // visiting its registers; `available` is that input's exact class row.
    pub(super) fn unconstrained(
        &mut self,
        class: &RegisterClass,
        available: &[RegisterViewId],
        block: SelectedBlockId,
        point: LiveRangePoint,
    ) -> Vec<RegisterViewId> {
        let cell = self
            .boundaries
            .partition_point(|boundary| *boundary <= point);
        self.candidates
            .entry((class.id, block, cell))
            .or_insert_with(|| {
                unconstrained(
                    class,
                    available,
                    block,
                    point,
                    self.function,
                    self.physical,
                    self.reservations,
                )
            })
            .clone()
    }

    pub(super) fn restrict_to_fixed(
        &self,
        function_index: usize,
        register: &VirtualLiveRange,
        block: SelectedBlockId,
        point: LiveRangePoint,
        fixed: Option<RegisterViewId>,
        candidates: &mut Vec<RegisterViewId>,
    ) -> Result<(), AllocationLegalityError> {
        restrict_to_fixed(
            function_index,
            register,
            block,
            point,
            fixed,
            self.function,
            self.physical,
            self.reservations,
            candidates,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn unconstrained(
    class: &RegisterClass,
    available: &[RegisterViewId],
    block: SelectedBlockId,
    point: LiveRangePoint,
    function: &FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    reservations: &ValidatedRegisterReservationProfile,
) -> Vec<RegisterViewId> {
    class
        .views
        .iter()
        .copied()
        .filter(|view_id| available.binary_search(view_id).is_ok())
        .filter(|view_id| {
            physical
                .model()
                .views
                .get(usize::from(view_id.0))
                .is_some_and(|view| {
                    view.id == *view_id
                        && view.allocatable
                        && !conflicts(view, block, point, function, reservations)
                })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn restrict_to_fixed(
    function_index: usize,
    register: &VirtualLiveRange,
    block: SelectedBlockId,
    point: LiveRangePoint,
    fixed: Option<RegisterViewId>,
    function: &FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    reservations: &ValidatedRegisterReservationProfile,
    candidates: &mut Vec<RegisterViewId>,
) -> Result<(), AllocationLegalityError> {
    let Some(fixed) = fixed else {
        return Ok(());
    };
    let Some(view) = physical.model().views.get(usize::from(fixed.0)) else {
        return Err(AllocationLegalityError::UnknownFixedView {
            function: function_index,
            register: register.virtual_register.0,
            view: fixed.0,
        });
    };
    if view.id != fixed || view.class != register.class {
        return Err(AllocationLegalityError::UnknownFixedView {
            function: function_index,
            register: register.virtual_register.0,
            view: fixed.0,
        });
    }
    if conflicts(view, block, point, function, reservations) {
        return Err(AllocationLegalityError::IllegalFixedView {
            function: function_index,
            register: register.virtual_register.0,
            view: fixed.0,
        });
    }
    candidates.clear();
    candidates.push(fixed);
    Ok(())
}

fn conflicts(
    view: &RegisterView,
    block: SelectedBlockId,
    point: LiveRangePoint,
    function: &FunctionLiveRanges,
    reservations: &ValidatedRegisterReservationProfile,
) -> bool {
    view.units.iter().chain(&view.write_units).any(|unit| {
        reservations.reserved_units().binary_search(unit).is_ok()
            || function
                .architectural_units
                .iter()
                .find(|row| row.unit == *unit)
                .is_some_and(|row| {
                    row.fragments.iter().any(|fragment| {
                        fragment.block == block && fragment.start <= point && point < fragment.end
                    }) || row
                        .actions
                        .iter()
                        .any(|action| action.block == block && action.point == point)
                })
    })
}

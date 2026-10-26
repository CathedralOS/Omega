//! Physical-view availability, reservation, and fixed-view filtering.

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
    candidates_by_class: Vec<Vec<Vec<(SelectedBlockId, Vec<RegisterViewId>)>>>,
    boundaries_by_class: Vec<Vec<LiveRangePoint>>,
}

impl<'a> CandidateViews<'a> {
    pub(super) fn new(
        function: &'a FunctionLiveRanges,
        physical: &'a ValidatedPhysicalRegisterModel,
        reservations: &'a ValidatedRegisterReservationProfile,
    ) -> Self {
        // Candidate availability for a register class can change only at a
        // fragment edge or one-phase action of a non-reserved unit that belongs
        // to an allocatable view of that class. Units outside the class (such
        // as flags clobbers on integer/float classes) or permanently reserved
        // units (such as the stack pointer) never alter unconstrained class
        // availability and must not subdivide every fragment into thousands of
        // identical cells.
        let model = physical.model();
        let reserved = reservations.reserved_units();
        let max_class_id = model
            .classes
            .iter()
            .map(|c| usize::from(c.id.0))
            .max()
            .unwrap_or(0);
        let mut boundaries_by_class = vec![Vec::new(); max_class_id + 1];
        let mut candidates_by_class = vec![Vec::new(); max_class_id + 1];
        for class in &model.classes {
            let mut class_units = std::collections::BTreeSet::new();
            for &view_id in &class.views {
                if let Some(view) = model.views.get(usize::from(view_id.0))
                    && view.id == view_id
                    && view.allocatable
                {
                    for &unit in view.units.iter().chain(&view.write_units) {
                        if reserved.binary_search(&unit).is_err() {
                            class_units.insert(unit);
                        }
                    }
                }
            }
            let mut boundaries = Vec::new();
            for unit in &function.architectural_units {
                if !class_units.contains(&unit.unit) {
                    continue;
                }
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
            let class_idx = usize::from(class.id.0);
            candidates_by_class[class_idx] = vec![Vec::new(); boundaries.len() + 1];
            boundaries_by_class[class_idx] = boundaries;
        }
        Self {
            function,
            physical,
            reservations,
            candidates_by_class,
            boundaries_by_class,
        }
    }

    #[inline]
    pub(super) fn boundaries_for(&self, class: RegisterClassId) -> &[LiveRangePoint] {
        self.boundaries_by_class
            .get(usize::from(class.0))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    // The function caller validates one immutable availability input before
    // visiting its registers; `available` is that input's exact class row.
    // Lend prepared candidates until a consumer publishes a new span. Adjacent
    // cells with equal candidates merge; cloning at every point allocates a
    // vector only to discard it when it equals the preceding span.
    pub(super) fn candidates_at(
        &mut self,
        function_index: usize,
        register: &VirtualLiveRange,
        class: &RegisterClass,
        available: &[RegisterViewId],
        block: SelectedBlockId,
        point: LiveRangePoint,
        fixed: Option<RegisterViewId>,
    ) -> Result<&[RegisterViewId], AllocationLegalityError> {
        let class_idx = usize::from(class.id.0);
        let cell = self.boundaries_by_class[class_idx]
            .partition_point(|boundary| *boundary <= point);
        self.candidates_at_cell(
            function_index,
            register,
            class,
            available,
            block,
            point,
            cell,
            fixed,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn candidates_at_cell(
        &mut self,
        function_index: usize,
        register: &VirtualLiveRange,
        class: &RegisterClass,
        available: &[RegisterViewId],
        block: SelectedBlockId,
        point: LiveRangePoint,
        cell: usize,
        fixed: Option<RegisterViewId>,
    ) -> Result<&[RegisterViewId], AllocationLegalityError> {
        let class_idx = usize::from(class.id.0);
        let bucket = &mut self.candidates_by_class[class_idx][cell];
        let pos = if let Some(pos) = bucket.iter().position(|(b, _)| *b == block) {
            pos
        } else {
            let computed = unconstrained(
                class,
                available,
                block,
                point,
                self.function,
                self.physical,
                self.reservations,
            );
            bucket.push((block, computed));
            bucket.len() - 1
        };
        let candidates = bucket[pos].1.as_slice();
        let Some(fixed) = fixed else {
            return Ok(candidates);
        };
        // A fixed view has always replaced (not intersected) the general
        // roster after its own class and conflict checks. Borrow its exact
        // model identity rather than clearing and rebuilding a one-item Vec.
        fixed_candidate(
            function_index,
            register,
            block,
            point,
            fixed,
            self.function,
            self.physical,
            self.reservations,
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
fn fixed_candidate<'physical>(
    function_index: usize,
    register: &VirtualLiveRange,
    block: SelectedBlockId,
    point: LiveRangePoint,
    fixed: RegisterViewId,
    function: &FunctionLiveRanges,
    physical: &'physical ValidatedPhysicalRegisterModel,
    reservations: &ValidatedRegisterReservationProfile,
) -> Result<&'physical [RegisterViewId], AllocationLegalityError> {
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
    Ok(std::slice::from_ref(&view.id))
}

fn conflicts(
    view: &RegisterView,
    block: SelectedBlockId,
    point: LiveRangePoint,
    function: &FunctionLiveRanges,
    reservations: &ValidatedRegisterReservationProfile,
) -> bool {
    view.units.iter().chain(&view.write_units).any(|unit| {
        if reservations.reserved_units().binary_search(unit).is_ok() {
            return true;
        }
        let Ok(unit_idx) = function
            .architectural_units
            .binary_search_by_key(unit, |row| row.unit)
        else {
            return false;
        };
        let row = &function.architectural_units[unit_idx];
        let frag_pos = row
            .fragments
            .partition_point(|fragment| fragment.start <= point);
        if frag_pos > 0 {
            let fragment = &row.fragments[frag_pos - 1];
            if fragment.block == block && point < fragment.end {
                return true;
            }
        }
        let act_pos = row.actions.partition_point(|action| action.point < point);
        row.actions[act_pos..]
            .iter()
            .take_while(|action| action.point == point)
            .any(|action| action.block == block)
    })
}

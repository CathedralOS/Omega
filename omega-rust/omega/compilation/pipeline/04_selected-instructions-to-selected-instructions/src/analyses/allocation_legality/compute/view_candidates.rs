//! Physical-view availability, reservation, and fixed-view filtering.

use register_model::{
    RegisterClass, RegisterClassId, RegisterView, RegisterViewId, ValidatedPhysicalRegisterModel,
    ValidatedRegisterReservationProfile,
};
use selected_instructions::SelectedBlockId;

use crate::{AllocationLegalityError, ValidatedAllocatorAvailability};
use selected_instructions::{
    ArchitecturalUnitLiveRange, FunctionLiveRanges, LiveRangePoint, VirtualLiveRange,
};

#[derive(Clone, Default)]
enum CellCandidates {
    #[default]
    Unknown,
    AllBaseViews,
    PerBlock(Vec<(SelectedBlockId, Vec<RegisterViewId>)>),
}

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
    base_views_by_class: Vec<Vec<RegisterViewId>>,
    base_view_masks_by_class: Vec<Option<Vec<(RegisterViewId, u64)>>>,
    arch_units_by_class: Vec<Vec<usize>>,
    cells_by_class: Vec<Vec<CellCandidates>>,
    boundaries_by_class: Vec<Vec<LiveRangePoint>>,
}

impl<'a> CandidateViews<'a> {
    pub(super) fn new(
        function: &'a FunctionLiveRanges,
        availability: &'a ValidatedAllocatorAvailability,
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
        let mut base_views_by_class = vec![Vec::new(); max_class_id + 1];
        let mut base_view_masks_by_class = vec![None; max_class_id + 1];
        let mut arch_units_by_class = vec![Vec::new(); max_class_id + 1];
        let mut cells_by_class = vec![Vec::new(); max_class_id + 1];
        let mut boundaries_by_class = vec![Vec::new(); max_class_id + 1];
        for class in &model.classes {
            let class_idx = usize::from(class.id.0);
            let available = availability
                .unconstrained_views(class.id)
                .unwrap_or(&[][..]);
            let mut class_units = Vec::new();
            let mut base_views = Vec::with_capacity(class.views.len());
            for &view_id in &class.views {
                if let Some(view) = model.views.get(usize::from(view_id.0))
                    && view.id == view_id
                    && view.allocatable
                {
                    let mut has_reserved = false;
                    for &unit in view.units.iter().chain(&view.write_units) {
                        if reserved.binary_search(&unit).is_err() {
                            if !class_units.contains(&unit) {
                                class_units.push(unit);
                            }
                        } else {
                            has_reserved = true;
                        }
                    }
                    if !has_reserved && available.binary_search(&view_id).is_ok() {
                        base_views.push(view_id);
                    }
                }
            }
            class_units.sort_unstable();
            let mut arch_units = Vec::new();
            let mut boundaries = Vec::new();
            for (unit_idx, unit) in function.architectural_units.iter().enumerate() {
                if class_units.binary_search(&unit.unit).is_err() {
                    continue;
                }
                arch_units.push(unit_idx);
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
            if arch_units.len() <= 64 {
                let mut masks = Vec::with_capacity(base_views.len());
                for &view_id in &base_views {
                    let view = &model.views[usize::from(view_id.0)];
                    let mut mask = 0u64;
                    for (bit, &unit_idx) in arch_units.iter().enumerate() {
                        let arch_unit = function.architectural_units[unit_idx].unit;
                        if view.units.contains(&arch_unit) || view.write_units.contains(&arch_unit)
                        {
                            mask |= 1u64 << bit;
                        }
                    }
                    masks.push((view_id, mask));
                }
                base_view_masks_by_class[class_idx] = Some(masks);
            }
            base_views_by_class[class_idx] = base_views;
            arch_units_by_class[class_idx] = arch_units;
            cells_by_class[class_idx] = vec![CellCandidates::Unknown; boundaries.len() + 1];
            boundaries_by_class[class_idx] = boundaries;
        }
        Self {
            function,
            physical,
            reservations,
            base_views_by_class,
            base_view_masks_by_class,
            arch_units_by_class,
            cells_by_class,
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
        if let Some(fixed) = fixed {
            // A fixed view has always replaced (not intersected) the general
            // roster after its own class and conflict checks. Borrow its exact
            // model identity rather than clearing and rebuilding a one-item Vec.
            return fixed_candidate(
                function_index,
                register,
                block,
                point,
                fixed,
                self.function,
                self.physical,
                self.reservations,
            );
        }
        let class_idx = usize::from(class.id.0);
        if matches!(self.cells_by_class[class_idx][cell], CellCandidates::Unknown) {
            let arch_units = &self.arch_units_by_class[class_idx];
            let any_active_at_point = arch_units.iter().any(|&unit_idx| {
                unit_active_at_point(&self.function.architectural_units[unit_idx], point)
            });
            if !any_active_at_point {
                self.cells_by_class[class_idx][cell] = CellCandidates::AllBaseViews;
            } else {
                self.cells_by_class[class_idx][cell] = CellCandidates::PerBlock(Vec::new());
            }
        }
        match &mut self.cells_by_class[class_idx][cell] {
            CellCandidates::AllBaseViews => Ok(&self.base_views_by_class[class_idx]),
            CellCandidates::PerBlock(bucket) => {
                let pos = if let Some(pos) = bucket.iter().position(|(b, _)| *b == block) {
                    pos
                } else {
                    let computed = if let Some(masks) = &self.base_view_masks_by_class[class_idx] {
                        let mut active_mask = 0u64;
                        for (bit, &unit_idx) in
                            self.arch_units_by_class[class_idx].iter().enumerate()
                        {
                            if unit_active_in_block_at_point(
                                &self.function.architectural_units[unit_idx],
                                block,
                                point,
                            ) {
                                active_mask |= 1u64 << bit;
                            }
                        }
                        if active_mask == 0 {
                            self.base_views_by_class[class_idx].clone()
                        } else {
                            masks
                                .iter()
                                .filter_map(|&(view_id, mask)| {
                                    ((mask & active_mask) == 0).then_some(view_id)
                                })
                                .collect()
                        }
                    } else {
                        unconstrained(
                            class,
                            available,
                            block,
                            point,
                            self.function,
                            self.physical,
                            self.reservations,
                        )
                    };
                    bucket.push((block, computed));
                    bucket.len() - 1
                };
                Ok(bucket[pos].1.as_slice())
            }
            CellCandidates::Unknown => unreachable!(),
        }
    }
}

fn unit_active_at_point(row: &ArchitecturalUnitLiveRange, point: LiveRangePoint) -> bool {
    let frag_pos = row
        .fragments
        .partition_point(|fragment| fragment.start <= point);
    if frag_pos > 0 && point < row.fragments[frag_pos - 1].end {
        return true;
    }
    row.actions
        .binary_search_by_key(&point, |action| action.point)
        .is_ok()
}

fn unit_active_in_block_at_point(
    row: &ArchitecturalUnitLiveRange,
    block: SelectedBlockId,
    point: LiveRangePoint,
) -> bool {
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

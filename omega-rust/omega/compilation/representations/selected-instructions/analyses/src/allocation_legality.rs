/*
  Exact physical-view allocation legality analysis (`analyze_allocation_legality`,
  `ValidatedAllocationLegality`, `AllocationLegalityValidationReceipt`).

  Compresses physical register view availability across each virtual live-range
  fragment into constant candidate spans while retaining every fixed-view,
  early-clobber, tied-operand, and architectural-unit constraint boundary.
*/

use crate::{ValidatedAllocatorAvailability, ValidatedLiveRanges};
use register_model::{
    PreparedRegisterConstraintCatalog, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterReservationProfile,
};

pub mod compute {

    mod early_clobbers {

        use register_model::{RegisterClass, RegisterViewId};

        use super::{fixed_views, view_candidates::CandidateViews};
        use crate::AllocationLegalityError;
        use register_homes::VirtualEarlyClobberPointLegality;
        use selected_instructions::{EarlyClobberConstraint, VirtualLiveRange};

        #[allow(clippy::too_many_arguments)]
        pub fn compute(
            function_index: usize,
            early_clobbers: &[&EarlyClobberConstraint],
            register: &VirtualLiveRange,
            class: &RegisterClass,
            available: &[RegisterViewId],
            views: &mut CandidateViews<'_>,
        ) -> Result<Vec<VirtualEarlyClobberPointLegality>, AllocationLegalityError> {
            if early_clobbers.is_empty() {
                return Ok(Vec::new());
            }
            let mut early_clobber_points = Vec::with_capacity(early_clobbers.len());
            for &early in early_clobbers {
                if early.def_class != register.class {
                    return Err(AllocationLegalityError::UnknownClass {
                        function: function_index,
                        register: register.virtual_register.0,
                        class: early.def_class.0,
                    });
                }
                let fixed = fixed_views::for_early_clobber(function_index, register, early)?;
                let candidates = views.candidates_at(
                    function_index,
                    register,
                    class,
                    available,
                    early.block,
                    early.early_point,
                    fixed,
                )?;
                if candidates.is_empty() {
                    return Err(AllocationLegalityError::NoCandidateViews {
                        function: function_index,
                        register: register.virtual_register.0,
                        block: early.block.0,
                        point: early.early_point.0,
                    });
                }
                early_clobber_points.push(VirtualEarlyClobberPointLegality {
                    block: early.block,
                    position: early.position,
                    instruction: early.instruction,
                    operand: early.def_operand,
                    point: early.early_point,
                    candidates: candidates.to_vec(),
                });
            }
            Ok(early_clobber_points)
        }
    }
    mod fixed_views {

        use std::collections::BTreeSet;

        use register_model::RegisterViewId;
        use selected_instructions::SelectedBlockId;

        use crate::AllocationLegalityError;
        use register_homes::EntryFixedViewTransition;
        use selected_instructions::{
            EarlyClobberConstraint, LiveRangePoint, VirtualFixedConstraintSite, VirtualLiveRange,
        };

        pub(super) fn for_early_clobber(
            function_index: usize,
            register: &VirtualLiveRange,
            early: &EarlyClobberConstraint,
        ) -> Result<Option<RegisterViewId>, AllocationLegalityError> {
            let fixed = register
                .fixed_constraints
                .iter()
                .filter_map(|constraint| match constraint.site {
                    VirtualFixedConstraintSite::Operand {
                        position,
                        instruction,
                        operand,
                        access: register_model::RegisterOperandAccess::Def,
                        ..
                    } if position == early.position
                        && instruction == early.instruction
                        && operand == early.def_operand =>
                    {
                        Some(constraint.view)
                    }
                    _ => None,
                })
                .collect::<BTreeSet<_>>();
            reject_ambiguous(function_index, register, fixed)
        }

        pub(super) fn at_live_point(
            function_index: usize,
            register: &VirtualLiveRange,
            block: SelectedBlockId,
            point: LiveRangePoint,
            entry_point: Option<(SelectedBlockId, LiveRangePoint)>,
        ) -> Result<Option<RegisterViewId>, AllocationLegalityError> {
            if register.fixed_constraints.is_empty() {
                return Ok(None);
            }
            let mut fixed = BTreeSet::new();
            for constraint in &register.fixed_constraints {
                let applies = match constraint.site {
                    VirtualFixedConstraintSite::Entry => entry_point == Some((block, point)),
                    VirtualFixedConstraintSite::Operand {
                        point: constraint_point,
                        ..
                    } => constraint_point == point,
                };
                if applies {
                    fixed.insert(constraint.view);
                }
            }
            reject_ambiguous(function_index, register, fixed)
        }

        pub(super) fn entry_transitions(
            register: &VirtualLiveRange,
        ) -> Vec<EntryFixedViewTransition> {
            if register.fixed_constraints.len() < 2 {
                return Vec::new();
            }
            let pinned = register
                .fixed_constraints
                .iter()
                .map(|constraint| constraint.view)
                .collect::<BTreeSet<_>>();
            if pinned.len() < 2 {
                return Vec::new();
            }
            let mut transitions = Vec::new();
            for constraint in &register.fixed_constraints {
                let site @ VirtualFixedConstraintSite::Operand {
                    access: register_model::RegisterOperandAccess::Use,
                    ..
                } = constraint.site
                else {
                    continue;
                };
                transitions.extend(
                    pinned
                        .iter()
                        .copied()
                        .filter(|from| *from != constraint.view)
                        .map(|from| EntryFixedViewTransition {
                            from_view: from,
                            to_site: site,
                            to_view: constraint.view,
                        }),
                );
            }
            transitions
        }

        fn reject_ambiguous(
            function_index: usize,
            register: &VirtualLiveRange,
            fixed: BTreeSet<RegisterViewId>,
        ) -> Result<Option<RegisterViewId>, AllocationLegalityError> {
            if fixed.len() > 1 {
                return Err(AllocationLegalityError::IllegalFixedView {
                    function: function_index,
                    register: register.virtual_register.0,
                    view: fixed.last().expect("two fixed views exist").0,
                });
            }
            Ok(fixed.into_iter().next())
        }
    }
    mod function {

        use register_model::{ValidatedPhysicalRegisterModel, ValidatedRegisterReservationProfile};

        use super::{early_clobbers, fixed_views, live_points, view_candidates::CandidateViews};
        use crate::{AllocationLegalityError, ValidatedAllocatorAvailability};
        use register_homes::{FunctionAllocationLegality, VirtualRegisterAllocationLegality};
        use selected_instructions::{
            ArchitecturalUnitLiveRange, FunctionLiveRanges, LiveRangePoint,
        };

        fn first_unit_diff_point(
            left: &ArchitecturalUnitLiveRange,
            right: &ArchitecturalUnitLiveRange,
        ) -> LiveRangePoint {
            let mut limit = LiveRangePoint(u32::MAX);
            for (a, b) in left.fragments.iter().zip(&right.fragments) {
                if a != b {
                    let pt = if a.block == b.block && a.start == b.start {
                        a.end.min(b.end)
                    } else {
                        a.start.min(b.start)
                    };
                    limit = limit.min(pt);
                    break;
                }
            }
            if left.fragments.len() != right.fragments.len() {
                let min_len = left.fragments.len().min(right.fragments.len());
                if let Some(extra) = left
                    .fragments
                    .get(min_len)
                    .or_else(|| right.fragments.get(min_len))
                {
                    limit = limit.min(extra.start);
                }
            }
            for (a, b) in left.actions.iter().zip(&right.actions) {
                if a != b {
                    limit = limit.min(a.point.min(b.point));
                    break;
                }
            }
            if left.actions.len() != right.actions.len() {
                let min_len = left.actions.len().min(right.actions.len());
                if let Some(extra) = left
                    .actions
                    .get(min_len)
                    .or_else(|| right.actions.get(min_len))
                {
                    limit = limit.min(extra.point);
                }
            }
            limit
        }

        fn unchanged_point_limit(
            previous: &FunctionLiveRanges,
            current: &FunctionLiveRanges,
        ) -> LiveRangePoint {
            let mut limit = match previous
                .block_domains
                .iter()
                .zip(&current.block_domains)
                .position(|(a, b)| a != b)
            {
                None if previous.block_domains.len() == current.block_domains.len() => {
                    LiveRangePoint(u32::MAX)
                }
                Some(0) => return LiveRangePoint(0),
                Some(diff_idx) => current.block_domains[diff_idx - 1].end,
                None => {
                    let min_len = previous
                        .block_domains
                        .len()
                        .min(current.block_domains.len());
                    if min_len == 0 {
                        return LiveRangePoint(0);
                    }
                    current.block_domains[min_len - 1].end
                }
            };
            let mut i = 0;
            let mut j = 0;
            while i < previous.architectural_units.len() && j < current.architectural_units.len() {
                let left = &previous.architectural_units[i];
                let right = &current.architectural_units[j];
                match left.unit.cmp(&right.unit) {
                    std::cmp::Ordering::Equal => {
                        if left != right {
                            limit = limit.min(first_unit_diff_point(left, right));
                        }
                        i += 1;
                        j += 1;
                    }
                    std::cmp::Ordering::Less => {
                        if let Some(f) = left.fragments.first() {
                            limit = limit.min(f.start);
                        }
                        if let Some(a) = left.actions.first() {
                            limit = limit.min(a.point);
                        }
                        i += 1;
                    }
                    std::cmp::Ordering::Greater => {
                        if let Some(f) = right.fragments.first() {
                            limit = limit.min(f.start);
                        }
                        if let Some(a) = right.actions.first() {
                            limit = limit.min(a.point);
                        }
                        j += 1;
                    }
                }
            }
            for left in &previous.architectural_units[i..] {
                if let Some(f) = left.fragments.first() {
                    limit = limit.min(f.start);
                }
                if let Some(a) = left.actions.first() {
                    limit = limit.min(a.point);
                }
            }
            for right in &current.architectural_units[j..] {
                if let Some(f) = right.fragments.first() {
                    limit = limit.min(f.start);
                }
                if let Some(a) = right.actions.first() {
                    limit = limit.min(a.point);
                }
            }
            limit
        }

        pub(crate) fn compute_reusing(
            function_index: usize,
            function: &FunctionLiveRanges,
            availability: &ValidatedAllocatorAvailability,
            physical: &ValidatedPhysicalRegisterModel,
            reservations: &ValidatedRegisterReservationProfile,
            previous: Option<(&FunctionLiveRanges, &FunctionAllocationLegality)>,
        ) -> Result<FunctionAllocationLegality, AllocationLegalityError> {
            let reuse_state = previous.and_then(|(prev_ranges, prev_legality)| {
                if prev_ranges.machine != function.machine
                    || prev_legality.machine != function.machine
                    || prev_ranges.virtual_registers.len() != prev_legality.virtual_registers.len()
                    || !function.early_clobbers.is_empty()
                    || !prev_ranges.early_clobbers.is_empty()
                {
                    return None;
                }
                let limit = unchanged_point_limit(prev_ranges, function);
                (limit > LiveRangePoint(0)).then_some((prev_ranges, prev_legality, limit))
            });
            let mut candidates =
                CandidateViews::new(function, availability, physical, reservations);
            let max_early_reg = function
                .early_clobbers
                .iter()
                .map(|early| early.def_virtual_register.0 as usize)
                .max()
                .unwrap_or(0);
            let mut early_by_register: Vec<Vec<&selected_instructions::EarlyClobberConstraint>> =
                if function.early_clobbers.is_empty() {
                    Vec::new()
                } else {
                    let mut grouped = vec![Vec::new(); max_early_reg + 1];
                    for early in &function.early_clobbers {
                        grouped[early.def_virtual_register.0 as usize].push(early);
                    }
                    grouped
                };
            let virtual_registers = function
                .virtual_registers
                .iter()
                .enumerate()
                .map(|(reg_index, register)| {
                    if let Some((prev_ranges, prev_legality, limit)) = reuse_state
                        && register.fragments.last().is_none_or(|f| f.end <= limit)
                        && let Some(prev_reg) = prev_ranges.virtual_registers.get(reg_index)
                        && let Some(prev_leg) = prev_legality.virtual_registers.get(reg_index)
                        && prev_reg.virtual_register == register.virtual_register
                        && prev_reg.class == register.class
                        && prev_leg.virtual_register == register.virtual_register
                        && prev_leg.class == register.class
                        && prev_reg.fragments == register.fragments
                        && prev_reg.fixed_constraints == register.fixed_constraints
                    {
                        return Ok(prev_leg.clone());
                    }
                    let class = physical
                        .model()
                        .classes
                        .get(usize::from(register.class.0))
                        .filter(|class| class.id == register.class)
                        .ok_or(AllocationLegalityError::UnknownClass {
                            function: function_index,
                            register: register.virtual_register.0,
                            class: register.class.0,
                        })?;
                    let available = availability.unconstrained_views(register.class).ok_or(
                        AllocationLegalityError::UnknownClass {
                            function: function_index,
                            register: register.virtual_register.0,
                            class: register.class.0,
                        },
                    )?;
                    let entry_point = register
                        .fragments
                        .first()
                        .map(|fragment| (fragment.block, fragment.start));
                    let spans = live_points::compute(
                        function_index,
                        register,
                        class,
                        available,
                        entry_point,
                        &mut candidates,
                    )?;
                    let early_slice = early_by_register
                        .get(register.virtual_register.0 as usize)
                        .map_or(&[][..], Vec::as_slice);
                    let early_clobber_points = early_clobbers::compute(
                        function_index,
                        early_slice,
                        register,
                        class,
                        available,
                        &mut candidates,
                    )?;
                    let entry_transitions = fixed_views::entry_transitions(register);
                    Ok(VirtualRegisterAllocationLegality {
                        virtual_register: register.virtual_register,
                        class: register.class,
                        spans,
                        early_clobber_points,
                        entry_transitions,
                    })
                })
                .collect::<Result<Vec<_>, AllocationLegalityError>>()?;
            let _ = &mut early_by_register;
            Ok(FunctionAllocationLegality {
                machine: function.machine,
                virtual_registers,
            })
        }
    }
    mod live_points {

        use register_model::{RegisterClass, RegisterViewId};
        use selected_instructions::SelectedBlockId;

        use super::{fixed_views, view_candidates::CandidateViews};
        use crate::AllocationLegalityError;
        use register_homes::VirtualLegalitySpan;
        use selected_instructions::{LiveRangePoint, VirtualFixedConstraintSite, VirtualLiveRange};

        #[allow(clippy::too_many_arguments)]
        pub fn compute(
            function_index: usize,
            register: &VirtualLiveRange,
            class: &RegisterClass,
            available: &[RegisterViewId],
            entry_point: Option<(SelectedBlockId, LiveRangePoint)>,
            views: &mut CandidateViews<'_>,
        ) -> Result<Vec<VirtualLegalitySpan>, AllocationLegalityError> {
            let mut boundaries = Vec::new();
            if !register.fixed_constraints.is_empty() {
                for constraint in &register.fixed_constraints {
                    let point = match constraint.site {
                        VirtualFixedConstraintSite::Entry => entry_point.map(|(_, point)| point),
                        VirtualFixedConstraintSite::Operand { point, .. } => Some(point),
                    };
                    if let Some(point) = point {
                        boundaries.push(point);
                        if let Some(end) = point.0.checked_add(1) {
                            boundaries.push(LiveRangePoint(end));
                        }
                    }
                }
                if boundaries.len() > 1 {
                    boundaries.sort_unstable();
                    boundaries.dedup();
                }
            }
            let mut spans: Vec<VirtualLegalitySpan> = Vec::with_capacity(register.fragments.len());
            for fragment in &register.fragments {
                if fragment.start >= fragment.end {
                    continue;
                }
                let fragment_offset = spans.len();
                let mut last_candidates_ptr: Option<(*const RegisterViewId, usize)> = None;
                let mut point = fragment.start;
                let mut architectural_index = views
                    .boundaries_for(class.id)
                    .partition_point(|boundary| *boundary <= point);
                let mut fixed_index = if boundaries.is_empty() {
                    0
                } else {
                    boundaries.partition_point(|boundary| *boundary <= point)
                };
                while point < fragment.end {
                    let cell = architectural_index;
                    let architectural = views
                        .boundaries_for(class.id)
                        .get(architectural_index)
                        .copied();
                    let fixed = boundaries.get(fixed_index).copied();
                    let end = architectural
                        .into_iter()
                        .chain(fixed)
                        .min()
                        .unwrap_or(fragment.end)
                        .min(fragment.end);
                    if architectural == Some(end) {
                        architectural_index += 1;
                    }
                    if fixed == Some(end) {
                        fixed_index += 1;
                    }
                    let fixed = if register.fixed_constraints.is_empty() {
                        None
                    } else {
                        fixed_views::at_live_point(
                            function_index,
                            register,
                            fragment.block,
                            point,
                            entry_point,
                        )?
                    };
                    let candidates = views.candidates_at_cell(
                        function_index,
                        register,
                        class,
                        available,
                        fragment.block,
                        point,
                        cell,
                        fixed,
                    )?;
                    if candidates.is_empty() {
                        return Err(AllocationLegalityError::NoCandidateViews {
                            function: function_index,
                            register: register.virtual_register.0,
                            block: fragment.block.0,
                            point: point.0,
                        });
                    }
                    let cand_key = (candidates.as_ptr(), candidates.len());
                    if spans.len() > fragment_offset
                        && (last_candidates_ptr == Some(cand_key)
                            || spans
                                .last()
                                .is_some_and(|span| span.candidates.as_slice() == candidates))
                    {
                        spans.last_mut().expect("current fragment has a span").end = end;
                    } else {
                        spans.push(VirtualLegalitySpan {
                            block: fragment.block,
                            start: point,
                            end,
                            candidates: candidates.to_vec(),
                        });
                    }
                    last_candidates_ptr = Some(cand_key);
                    point = end;
                }
            }
            Ok(spans)
        }
    }
    mod view_candidates {

        use register_model::{
            RegisterClass, RegisterClassId, RegisterView, RegisterViewId,
            ValidatedPhysicalRegisterModel, ValidatedRegisterReservationProfile,
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
            pub fn new(
                function: &'a FunctionLiveRanges,
                availability: &'a ValidatedAllocatorAvailability,
                physical: &'a ValidatedPhysicalRegisterModel,
                reservations: &'a ValidatedRegisterReservationProfile,
            ) -> Self {
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
                                if view.units.contains(&arch_unit)
                                    || view.write_units.contains(&arch_unit)
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
                if matches!(
                    self.cells_by_class[class_idx][cell],
                    CellCandidates::Unknown
                ) {
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
                            let computed =
                                if let Some(masks) = &self.base_view_masks_by_class[class_idx] {
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
                unit_active_in_block_at_point(&function.architectural_units[unit_idx], block, point)
            })
        }
    }

    pub(crate) use function::compute_reusing as compute_function_reusing;

    use register_model::{
        PreparedRegisterConstraintCatalog, TargetRegisterEnvironmentConstraintKeys,
        TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
        ValidatedRegisterReservationProfile, target_register_environment_identity,
    };

    use crate::{
        AllocationLegalityError, ValidatedAllocationLegality, ValidatedAllocatorAvailability,
        ValidatedLiveRanges,
    };
    use register_homes::AllocationLegalityPlan;

    pub(crate) fn compute_terminal_allocation_legality(
        ranges: &ValidatedLiveRanges,
        availability: &ValidatedAllocatorAvailability,
        register_environment: TargetRegisterEnvironmentIdentity,
        physical: &ValidatedPhysicalRegisterModel,
        constraints: &PreparedRegisterConstraintCatalog,
        reservations: &ValidatedRegisterReservationProfile,
        selected_keys: &TargetRegisterEnvironmentConstraintKeys,
        previous: Option<(&ValidatedLiveRanges, &ValidatedAllocationLegality)>,
    ) -> Result<(AllocationLegalityPlan, Vec<bool>), AllocationLegalityError> {
        validate_roots(
            ranges,
            availability,
            register_environment,
            physical,
            constraints,
            reservations,
        )?;
        let environment = target_register_environment_identity(
            ranges.plan().target,
            physical,
            constraints,
            reservations,
            selected_keys,
        );
        if environment != register_environment {
            return Err(AllocationLegalityError::RootMismatch);
        }
        let previous = previous.filter(|(prior_ranges, prior_legality)| {
            prior_ranges.plan().target == ranges.plan().target
                && prior_ranges.plan().optimization_unit == ranges.plan().optimization_unit
                && prior_ranges.plan().work_schedule == ranges.plan().work_schedule
                && prior_legality.plan().ranges == prior_ranges.receipt().identity()
                && prior_legality.receipt().ranges() == prior_ranges.receipt().identity()
                && prior_legality.plan().register_environment == register_environment
                && prior_legality.receipt().register_environment() == register_environment
                && prior_legality.plan().allocator_availability == availability.receipt().identity()
                && prior_legality.receipt().allocator_availability()
                    == availability.receipt().identity()
        });
        let mut carried = vec![false; ranges.plan().functions.len()];
        let functions = ranges
            .plan()
            .functions
            .iter()
            .enumerate()
            .map(|(function_index, live_ranges)| {
                if let Some((prior_ranges, prior_legality)) = previous
                    && prior_ranges.plan().functions.get(function_index) == Some(live_ranges)
                    && let Some(function) = prior_legality.plan().functions.get(function_index)
                {
                    carried[function_index] = true;
                    return Ok(function.clone());
                }
                let prior_pair = previous.and_then(|(prior_ranges, prior_legality)| {
                    Some((
                        prior_ranges.plan().functions.get(function_index)?.as_ref(),
                        prior_legality
                            .plan()
                            .functions
                            .get(function_index)?
                            .as_ref(),
                    ))
                });
                function::compute_reusing(
                    function_index,
                    live_ranges,
                    availability,
                    physical,
                    reservations,
                    prior_pair,
                )
                .map(std::sync::Arc::new)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((
            AllocationLegalityPlan {
                ranges: ranges.receipt().identity(),
                register_environment,
                allocator_availability: availability.receipt().identity(),
                functions,
            },
            carried,
        ))
    }

    fn validate_roots(
        ranges: &ValidatedLiveRanges,
        availability: &ValidatedAllocatorAvailability,
        register_environment: TargetRegisterEnvironmentIdentity,
        physical: &ValidatedPhysicalRegisterModel,
        constraints: &PreparedRegisterConstraintCatalog,
        reservations: &ValidatedRegisterReservationProfile,
    ) -> Result<(), AllocationLegalityError> {
        let physical_identity = physical.identity();
        if ranges.plan().target.architecture != physical.model().architecture
            || constraints.physical_identity() != physical_identity
            || reservations.physical_identity() != physical_identity
            || reservations.target() != ranges.plan().target
            || availability.receipt().register_environment() != register_environment
            || availability.receipt().physical() != physical_identity
        {
            return Err(AllocationLegalityError::RootMismatch);
        }
        Ok(())
    }
}
pub(crate) mod validate {
    mod counts {

        use crate::AllocationLegalityError;
        use register_homes::FunctionAllocationLegality;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct FunctionCounts {
            pub(super) virtual_register_count: usize,
            pub(super) point_count: usize,
            pub(super) candidate_count: usize,
            pub(super) entry_transition_count: usize,
        }

        impl FunctionCounts {
            pub fn new(
                function: &FunctionAllocationLegality,
                function_index: usize,
            ) -> Result<Self, AllocationLegalityError> {
                let overflow = || AllocationLegalityError::PointOverflow {
                    function: function_index,
                };
                let mut point_count = 0usize;
                let mut candidate_count = 0usize;
                for span in function
                    .virtual_registers
                    .iter()
                    .flat_map(|register| &register.spans)
                {
                    let width =
                        usize::try_from(span.end.0 - span.start.0).map_err(|_| overflow())?;
                    point_count = point_count.checked_add(width).ok_or_else(overflow)?;
                    let candidates = width
                        .checked_mul(span.candidates.len())
                        .ok_or_else(overflow)?;
                    candidate_count = candidate_count
                        .checked_add(candidates)
                        .ok_or_else(overflow)?;
                }
                Ok(Self {
                    virtual_register_count: function.virtual_registers.len(),
                    point_count,
                    candidate_count,
                    entry_transition_count: function
                        .virtual_registers
                        .iter()
                        .map(|register| register.entry_transitions.len())
                        .sum(),
                })
            }
        }
    }
    pub(crate) use counts::FunctionCounts;

    use crate::{
        AllocationLegalityError, AllocationLegalityValidationReceipt, ValidatedAllocationLegality,
    };
    use register_homes::{
        AllocationLegalityPlan, allocation_legality_function_digest,
        allocation_legality_identity_from_digests,
    };
    pub fn admit(
        plan: AllocationLegalityPlan,
        carried: &[bool],
        previous_digests: &[[u8; 32]],
        previous_counts: &[FunctionCounts],
    ) -> Result<ValidatedAllocationLegality, AllocationLegalityError> {
        let mut point_count = 0usize;
        let mut candidate_count = 0usize;
        let mut function_counts = Vec::with_capacity(plan.functions.len());
        for (function_index, function) in plan.functions.iter().enumerate() {
            let overflow = || AllocationLegalityError::PointOverflow {
                function: function_index,
            };
            let counts = match previous_counts.get(function_index) {
                Some(counts) if carried.get(function_index) == Some(&true) => *counts,
                _ => FunctionCounts::new(function, function_index)?,
            };
            point_count = point_count
                .checked_add(counts.point_count)
                .ok_or_else(overflow)?;
            candidate_count = candidate_count
                .checked_add(counts.candidate_count)
                .ok_or_else(overflow)?;
            function_counts.push(counts);
        }
        let function_digests =
            crate::carried_digests(&plan.functions, carried, previous_digests, |function| {
                allocation_legality_function_digest(function)
            });
        let identity = allocation_legality_identity_from_digests(&plan, &function_digests);
        let receipt = AllocationLegalityValidationReceipt {
            identity,
            ranges: plan.ranges,
            register_environment: plan.register_environment,
            allocator_availability: plan.allocator_availability,
            point_count,
            candidate_count,
            virtual_register_count: function_counts
                .iter()
                .map(|counts| counts.virtual_register_count)
                .sum(),
            entry_transition_count: function_counts
                .iter()
                .map(|counts| counts.entry_transition_count)
                .sum(),
        };
        Ok(ValidatedAllocationLegality {
            plan: plan.into(),
            receipt,
            function_digests,
            function_counts: function_counts.into(),
        })
    }
}

use register_homes::{
    AllocationLegalityIdentity, AllocationLegalityPlan, AllocatorAvailabilityIdentity,
};
use selected_instructions::LiveRangeIdentity;

pub fn analyze_allocation_legality(
    ranges: &ValidatedLiveRanges,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &PreparedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
) -> Result<ValidatedAllocationLegality, AllocationLegalityError> {
    let (plan, _) = compute::compute_terminal_allocation_legality(
        ranges,
        availability,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        None,
    )?;
    validate::admit(plan, &[], &[], &[])
}

pub fn analyze_allocation_legality_reusing(
    previous_ranges: &ValidatedLiveRanges,
    previous_legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &PreparedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
) -> Result<ValidatedAllocationLegality, AllocationLegalityError> {
    let (plan, carried) = compute::compute_terminal_allocation_legality(
        ranges,
        availability,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        Some((previous_ranges, previous_legality)),
    )?;
    validate::admit(
        plan,
        &carried,
        &previous_legality.function_digests,
        &previous_legality.function_counts,
    )
}

copy_receipt! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct AllocationLegalityValidationReceipt {
        pub(crate) identity: AllocationLegalityIdentity,
        pub(crate) ranges: LiveRangeIdentity,
        pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
        pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
        pub(crate) virtual_register_count: usize,
        pub(crate) point_count: usize,
        pub(crate) candidate_count: usize,
        pub(crate) entry_transition_count: usize,
    }
}

shared_plan_carrier!(
    ValidatedAllocationLegality: AllocationLegalityPlan, AllocationLegalityValidationReceipt {
        pub(crate) function_counts: std::sync::Arc<[validate::FunctionCounts]>,
    }
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationLegalityError {
    RootMismatch,
    UnknownClass {
        function: usize,
        register: u32,
        class: u16,
    },
    UnknownFixedView {
        function: usize,
        register: u32,
        view: u16,
    },
    IllegalFixedView {
        function: usize,
        register: u32,
        view: u16,
    },
    NoCandidateViews {
        function: usize,
        register: u32,
        block: u32,
        point: u32,
    },
    PointOverflow {
        function: usize,
    },
    FunctionMismatch {
        function: usize,
    },
    VirtualRegisterMismatch {
        function: usize,
        register: u32,
    },
    NonCanonicalRows {
        function: usize,
        register: u32,
    },
}

failed_display!(
    AllocationLegalityError,
    "Terminal allocation-legality derivation failed"
);

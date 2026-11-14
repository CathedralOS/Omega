/*
  Deterministic physical view assignment for fixed/precolored source-segment
  domains (`assign_fixed_precolored_segment_homes`,
  `ValidatedFixedPrecoloredSegmentHomes`,
  `FixedPrecoloredSegmentHomeValidationReceipt`).

  Assigns one deterministic physical register view to each authenticated
  source-segment domain while checking interference and conflict boundaries.
*/

mod compute {

    mod conflicts {
        use std::collections::{BTreeMap, BTreeSet};

        use register_model::{RegisterView, RegisterViewId, ValidatedPhysicalRegisterModel};

        use crate::FixedPrecoloredSegmentHomeError;
        use register_homes::FixedPrecoloredHomeDomainId;
        use selected_instructions::{
            FunctionLiveRanges, VirtualInterference, VirtualInterferences,
        };

        use super::super::domains::Domain;
        use crate::work_accounting::Work;

        type DomainPair = (FixedPrecoloredHomeDomainId, FixedPrecoloredHomeDomainId);
        type ViewConflict = (
            FixedPrecoloredHomeDomainId,
            RegisterViewId,
            FixedPrecoloredHomeDomainId,
            RegisterViewId,
        );

        pub(super) struct Conflicts {
            constrained: BTreeSet<DomainPair>,
            views: BTreeSet<ViewConflict>,
            conflicting: BTreeMap<DomainView, Vec<DomainView>>,
        }

        type DomainView = (FixedPrecoloredHomeDomainId, RegisterViewId);

        impl Conflicts {
            pub(super) fn domains(
                &self,
                left: FixedPrecoloredHomeDomainId,
                right: FixedPrecoloredHomeDomainId,
            ) -> bool {
                self.constrained.contains(&ordered_domains(left, right))
            }

            pub(super) fn views(
                &self,
                left_domain: FixedPrecoloredHomeDomainId,
                left_view: RegisterViewId,
                right_domain: FixedPrecoloredHomeDomainId,
                right_view: RegisterViewId,
            ) -> bool {
                self.views.contains(&ordered_views(
                    left_domain,
                    left_view,
                    right_domain,
                    right_view,
                ))
            }

            pub(super) fn conflicting(
                &self,
                domain: FixedPrecoloredHomeDomainId,
                view: RegisterViewId,
            ) -> &[DomainView] {
                self.conflicting
                    .get(&(domain, view))
                    .map(Vec::as_slice)
                    .unwrap_or(&[])
            }
        }

        pub(super) fn build(
            function: usize,
            domains: &[Domain],
            ranges: &FunctionLiveRanges,
            physical: &ValidatedPhysicalRegisterModel,
            work: &mut Work,
        ) -> Result<Conflicts, FixedPrecoloredSegmentHomeError> {
            let mut constrained = BTreeSet::new();
            let mut views = BTreeSet::new();
            let mut conflicting = BTreeMap::<DomainView, Vec<DomainView>>::new();
            for left_index in 0..domains.len() {
                for right_index in left_index + 1..domains.len() {
                    work.validation_steps(1)?;
                    work.iterations(1)?;
                    let left = &domains[left_index];
                    let right = &domains[right_index];
                    if !live_overlap(left, right)
                        || !registers_interfere(left, right, &ranges.interference)
                    {
                        continue;
                    }
                    constrained.insert((left.id, right.id));
                    for &left_candidate in &left.candidates {
                        let left_view = checked_view(function, left, left_candidate, physical)?;
                        for &right_candidate in &right.candidates {
                            work.candidates(1)?;
                            work.validation_steps(1)?;
                            let right_view =
                                checked_view(function, right, right_candidate, physical)?;
                            if left_view.footprints_overlap(right_view) {
                                views.insert((left.id, left_candidate, right.id, right_candidate));
                                conflicting
                                    .entry((left.id, left_candidate))
                                    .or_default()
                                    .push((right.id, right_candidate));
                                conflicting
                                    .entry((right.id, right_candidate))
                                    .or_default()
                                    .push((left.id, left_candidate));
                            }
                        }
                    }
                }
            }
            Ok(Conflicts {
                constrained,
                views,
                conflicting,
            })
        }

        fn live_overlap(left: &Domain, right: &Domain) -> bool {
            left.segments.iter().any(|left| {
                right.segments.iter().any(|right| {
                    left.block == right.block && left.start < right.end && right.start < left.end
                })
            })
        }

        fn registers_interfere(
            left: &Domain,
            right: &Domain,
            interference: &VirtualInterferences,
        ) -> bool {
            if left.virtual_register == right.virtual_register {
                return false;
            }
            let (lower, higher) = if left.virtual_register < right.virtual_register {
                (left.virtual_register, right.virtual_register)
            } else {
                (right.virtual_register, left.virtual_register)
            };
            interference
                .binary_search(&VirtualInterference { lower, higher })
                .is_ok()
        }

        fn checked_view<'a>(
            function: usize,
            domain: &Domain,
            candidate: RegisterViewId,
            physical: &'a ValidatedPhysicalRegisterModel,
        ) -> Result<&'a RegisterView, FixedPrecoloredSegmentHomeError> {
            physical
                .model()
                .views
                .iter()
                .find(|view| view.id == candidate && view.class == domain.class)
                .ok_or(FixedPrecoloredSegmentHomeError::UnknownOrIncompatibleView {
                    function,
                    register: domain.virtual_register.0,
                    view: candidate.0,
                })
        }

        fn ordered_domains(
            left: FixedPrecoloredHomeDomainId,
            right: FixedPrecoloredHomeDomainId,
        ) -> DomainPair {
            if left < right {
                (left, right)
            } else {
                (right, left)
            }
        }

        fn ordered_views(
            left_domain: FixedPrecoloredHomeDomainId,
            left_view: RegisterViewId,
            right_domain: FixedPrecoloredHomeDomainId,
            right_view: RegisterViewId,
        ) -> ViewConflict {
            if left_domain < right_domain {
                (left_domain, left_view, right_domain, right_view)
            } else {
                (right_domain, right_view, left_domain, left_view)
            }
        }
    }

    mod functions {
        use register_model::ValidatedPhysicalRegisterModel;

        use crate::FixedPrecoloredSegmentHomeError;
        use register_homes::{
            FunctionFixedPrecoloredSegmentHomes, FunctionFixedPrecoloredSplitRequirements,
        };
        use selected_instructions::FunctionLiveRanges;

        use super::super::domains;
        use super::{conflicts, placement};
        use crate::work_accounting::Work;

        pub(super) fn derive(
            requirements: &[FunctionFixedPrecoloredSplitRequirements],
            ranges: &[std::sync::Arc<FunctionLiveRanges>],
            physical: &ValidatedPhysicalRegisterModel,
            work: &mut Work,
        ) -> Result<Vec<FunctionFixedPrecoloredSegmentHomes>, FixedPrecoloredSegmentHomeError>
        {
            if requirements.len() != ranges.len() {
                return Err(FixedPrecoloredSegmentHomeError::RootMismatch);
            }
            requirements
                .iter()
                .zip(ranges)
                .enumerate()
                .map(|(function, (requirements, ranges))| {
                    if requirements.machine != ranges.machine {
                        return Err(FixedPrecoloredSegmentHomeError::FunctionMismatch { function });
                    }
                    let domains = domains::build(function, requirements, work)?;
                    let conflicts = conflicts::build(function, &domains, ranges, physical, work)?;
                    placement::assign(function, ranges.machine, &domains, &conflicts, ranges, work)
                })
                .collect()
        }
    }
    mod placement {
        use super::super::affinity_edges;
        use std::cmp::Reverse;
        use std::collections::{BTreeMap, BTreeSet};

        use register_model::RegisterViewId;

        use crate::FixedPrecoloredSegmentHomeError;
        use register_homes::{
            FixedPrecoloredHomeDomainId, FixedPrecoloredSourceSegmentHome,
            FunctionFixedPrecoloredSegmentHomes,
        };
        use selected_instructions::FunctionLiveRanges;

        use super::super::domains::Domain;
        use super::conflicts::Conflicts;
        use crate::work_accounting::Work;

        pub(super) fn assign(
            function: usize,
            machine: semantic_vocabulary::MachineId,
            domains: &[Domain],
            conflicts: &Conflicts,
            ranges: &FunctionLiveRanges,
            work: &mut Work,
        ) -> Result<FunctionFixedPrecoloredSegmentHomes, FixedPrecoloredSegmentHomeError> {
            let partners = affinity_edges(domains, ranges, work)?;
            let mut unassigned = (0..domains.len()).collect::<Vec<_>>();
            let mut assigned = BTreeMap::<usize, RegisterViewId>::new();
            let mut viables = BTreeMap::<usize, Vec<RegisterViewId>>::new();
            let mut by_id = BTreeMap::<FixedPrecoloredHomeDomainId, usize>::new();
            for (index, domain) in domains.iter().enumerate() {
                viables.insert(index, domain.candidates.clone());
                by_id.insert(domain.id, index);
            }
            while !unassigned.is_empty() {
                let (position, domain_index) = select(&unassigned, &viables, domains, conflicts);
                unassigned.remove(position);
                let viable = viables[&domain_index].clone();
                let view = preferred_view(
                    domain_index,
                    &viable,
                    &viables,
                    &unassigned,
                    &assigned,
                    domains,
                    conflicts,
                    &partners,
                    work,
                )?
                .ok_or_else(|| {
                    let domain = &domains[domain_index];
                    FixedPrecoloredSegmentHomeError::SegmentPressure {
                        function,
                        register: domain.virtual_register.0,
                        segment: domain.first_segment().0,
                    }
                })?;
                assigned.insert(domain_index, view);
                viables.remove(&domain_index);
                for &(neighbor, conflicting_view) in
                    conflicts.conflicting(domains[domain_index].id, view)
                {
                    work.candidates(1)?;
                    work.validation_steps(1)?;
                    work.iterations(1)?;
                    let Some(keeps) = by_id
                        .get(&neighbor)
                        .and_then(|index| viables.get_mut(index))
                    else {
                        continue;
                    };
                    if let Some(position) = keeps
                        .iter()
                        .position(|&candidate| candidate == conflicting_view)
                    {
                        keeps.remove(position);
                    }
                }
            }
            let mut assignments = domains
                .iter()
                .enumerate()
                .flat_map(|(domain_index, domain)| {
                    let view = assigned[&domain_index];
                    domain
                        .segments
                        .iter()
                        .map(move |segment| FixedPrecoloredSourceSegmentHome {
                            virtual_register: domain.virtual_register,
                            class: domain.class,
                            source_segment: segment.id,
                            allocation_domain: domain.id,
                            view,
                        })
                })
                .collect::<Vec<_>>();
            assignments
                .sort_by_key(|assignment| (assignment.virtual_register, assignment.source_segment));
            Ok(FunctionFixedPrecoloredSegmentHomes {
                machine,
                assignments,
            })
        }

        fn select(
            unassigned: &[usize],
            viables: &BTreeMap<usize, Vec<RegisterViewId>>,
            domains: &[Domain],
            conflicts: &Conflicts,
        ) -> (usize, usize) {
            let mut selected = None::<(usize, usize, usize)>;
            for (position, &domain_index) in unassigned.iter().enumerate() {
                let domain = &domains[domain_index];
                let degree = unassigned
                    .iter()
                    .copied()
                    .filter(|&other| {
                        other != domain_index && conflicts.domains(domain.id, domains[other].id)
                    })
                    .count();
                let viable_len = viables[&domain_index].len();
                let replace = match &selected {
                    None => true,
                    Some((_, best_index, best_degree)) => {
                        let best = &domains[*best_index];
                        (
                            viable_len,
                            Reverse(degree),
                            domain.first_point(),
                            domain.virtual_register,
                            domain.first_segment(),
                        ) < (
                            viables[best_index].len(),
                            Reverse(*best_degree),
                            best.first_point(),
                            best.virtual_register,
                            best.first_segment(),
                        )
                    }
                };
                if replace {
                    selected = Some((position, domain_index, degree));
                }
            }
            selected
                .map(|(position, domain_index, _)| (position, domain_index))
                .expect("nonempty unassigned domain roster")
        }

        #[allow(clippy::too_many_arguments)]
        fn preferred_view(
            domain_index: usize,
            viable: &[RegisterViewId],
            viables: &BTreeMap<usize, Vec<RegisterViewId>>,
            unassigned: &[usize],
            assigned: &BTreeMap<usize, RegisterViewId>,
            domains: &[Domain],
            conflicts: &Conflicts,
            incident: &BTreeMap<usize, Vec<(usize, usize)>>,
            work: &mut Work,
        ) -> Result<Option<RegisterViewId>, FixedPrecoloredSegmentHomeError> {
            let residual = if viable.len() > 1 {
                residual_base(domain_index, unassigned, viables, domains, conflicts, work)?
            } else {
                None
            };
            let mut keeping = Vec::with_capacity(viable.len());
            if let Some(residual) = &residual {
                for &view in viable {
                    if !strands_neighbor(domain_index, view, residual, domains, conflicts, work)? {
                        keeping.push(view);
                    }
                }
            }
            let pool: &[RegisterViewId] = if keeping.is_empty() {
                viable
            } else {
                keeping.as_slice()
            };
            let outlooks = partner_outlooks(
                domain_index,
                viables,
                unassigned,
                assigned,
                incident,
                domains,
                conflicts,
                work,
            )?;
            let partner_views = assigned_partner_views(
                domain_index,
                unassigned,
                assigned,
                incident,
                domains,
                conflicts,
                work,
            )?;
            let mut leading = None::<(usize, usize, Reverse<usize>, RegisterViewId)>;
            for &view in pool {
                let mut guaranteed = 0usize;
                let mut votes = 0usize;
                for &(lower, upper) in incident.get(&domain_index).map_or(&[][..], Vec::as_slice) {
                    work.validation_steps(1)?;
                    work.iterations(1)?;
                    let partner = if lower == domain_index { upper } else { lower };
                    if assigned.get(&partner) == Some(&view) {
                        guaranteed += 1;
                    } else if let Some(outlook) = outlooks.get(&partner)
                        && viables
                            .get(&partner)
                            .is_some_and(|viable| viable.binary_search(&view).is_ok())
                    {
                        let drawn =
                            outlook.assigned.get(&view).copied().unwrap_or(0) + outlook.pending;
                        if drawn > outlook.peak
                            || (drawn == outlook.peak
                                && outlook.peak_view.is_some_and(|peak| view < peak))
                        {
                            votes += 1;
                        }
                    }
                }
                let stolen = stolen_coalesces(
                    domain_index,
                    view,
                    unassigned,
                    viables,
                    &partner_views,
                    domains,
                    conflicts,
                    work,
                )?;
                let rank = (guaranteed, votes, Reverse(stolen));
                if leading.is_none_or(|(best_guaranteed, best_votes, best_stolen, _)| {
                    rank > (best_guaranteed, best_votes, best_stolen)
                }) {
                    leading = Some((guaranteed, votes, Reverse(stolen), view));
                }
            }
            Ok(leading
                .map(|(_, _, _, view)| view)
                .or_else(|| pool.first().copied()))
        }

        fn assigned_partner_views(
            domain_index: usize,
            unassigned: &[usize],
            assigned: &BTreeMap<usize, RegisterViewId>,
            incident: &BTreeMap<usize, Vec<(usize, usize)>>,
            domains: &[Domain],
            conflicts: &Conflicts,
            work: &mut Work,
        ) -> Result<BTreeMap<usize, BTreeMap<RegisterViewId, usize>>, FixedPrecoloredSegmentHomeError>
        {
            let mut views = BTreeMap::<usize, BTreeMap<RegisterViewId, usize>>::new();
            for &member in unassigned {
                if member == domain_index
                    || !conflicts.domains(domains[member].id, domains[domain_index].id)
                {
                    continue;
                }
                for &(lower, upper) in incident.get(&member).map_or(&[][..], Vec::as_slice) {
                    work.validation_steps(1)?;
                    work.iterations(1)?;
                    let other = if lower == member { upper } else { lower };
                    if let Some(&home) = assigned.get(&other) {
                        *views.entry(member).or_default().entry(home).or_default() += 1;
                    }
                }
            }
            Ok(views)
        }

        #[allow(clippy::too_many_arguments)]
        fn stolen_coalesces(
            domain_index: usize,
            view: RegisterViewId,
            unassigned: &[usize],
            viables: &BTreeMap<usize, Vec<RegisterViewId>>,
            partner_views: &BTreeMap<usize, BTreeMap<RegisterViewId, usize>>,
            domains: &[Domain],
            conflicts: &Conflicts,
            work: &mut Work,
        ) -> Result<usize, FixedPrecoloredSegmentHomeError> {
            let mut stolen = 0;
            for &neighbor in unassigned {
                if neighbor == domain_index
                    || !conflicts.domains(domains[neighbor].id, domains[domain_index].id)
                    || viables
                        .get(&neighbor)
                        .is_none_or(|viable| viable.binary_search(&view).is_err())
                {
                    continue;
                }
                work.candidates(1)?;
                work.validation_steps(1)?;
                work.iterations(1)?;
                if !conflicts.views(domains[neighbor].id, view, domains[domain_index].id, view) {
                    continue;
                }
                stolen += partner_views
                    .get(&neighbor)
                    .and_then(|views| views.get(&view))
                    .copied()
                    .unwrap_or(0);
            }
            Ok(stolen)
        }

        #[derive(Default)]
        struct PartnerOutlook {
            pending: usize,
            assigned: BTreeMap<RegisterViewId, usize>,
            peak: usize,
            peak_view: Option<RegisterViewId>,
        }

        #[allow(clippy::too_many_arguments)]
        fn partner_outlooks(
            domain_index: usize,
            viables: &BTreeMap<usize, Vec<RegisterViewId>>,
            unassigned: &[usize],
            assigned: &BTreeMap<usize, RegisterViewId>,
            incident: &BTreeMap<usize, Vec<(usize, usize)>>,
            domains: &[Domain],
            conflicts: &Conflicts,
            work: &mut Work,
        ) -> Result<BTreeMap<usize, PartnerOutlook>, FixedPrecoloredSegmentHomeError> {
            let mut outlooks = BTreeMap::<usize, PartnerOutlook>::new();
            for &(lower, upper) in incident.get(&domain_index).map_or(&[][..], Vec::as_slice) {
                work.validation_steps(1)?;
                work.iterations(1)?;
                let partner = if lower == domain_index { upper } else { lower };
                if partner != domain_index
                    && unassigned.contains(&partner)
                    && !conflicts.domains(domains[partner].id, domains[domain_index].id)
                {
                    outlooks.entry(partner).or_default().pending += 1;
                }
            }
            for (&member, outlook) in &mut outlooks {
                for &(lower, upper) in incident.get(&member).map_or(&[][..], Vec::as_slice) {
                    work.validation_steps(1)?;
                    work.iterations(1)?;
                    let other = if lower == member { upper } else { lower };
                    if let Some(&home) = assigned.get(&other) {
                        *outlook.assigned.entry(home).or_default() += 1;
                    }
                }
            }
            for (partner_domain, outlook) in &mut outlooks {
                let Some(viable) = viables.get(partner_domain) else {
                    continue;
                };
                for &view in viable {
                    let count = outlook.assigned.get(&view).copied().unwrap_or(0);
                    if outlook.peak_view.is_none() || count > outlook.peak {
                        outlook.peak = count;
                        outlook.peak_view = Some(view);
                    }
                }
            }
            Ok(outlooks)
        }

        fn residual_base(
            domain_index: usize,
            unassigned: &[usize],
            viables: &BTreeMap<usize, Vec<RegisterViewId>>,
            domains: &[Domain],
            conflicts: &Conflicts,
            work: &mut Work,
        ) -> Result<
            Option<BTreeMap<FixedPrecoloredHomeDomainId, Vec<RegisterViewId>>>,
            FixedPrecoloredSegmentHomeError,
        > {
            let mut retained = BTreeMap::<FixedPrecoloredHomeDomainId, Vec<RegisterViewId>>::new();
            for &neighbor in unassigned {
                if neighbor == domain_index {
                    continue;
                }
                let Some(neighbor_viable) = viables.get(&neighbor) else {
                    continue;
                };
                if !neighbor_viable.is_empty() {
                    retained.insert(domains[neighbor].id, neighbor_viable.clone());
                }
            }
            let mut forced = Vec::new();
            let mut queued = BTreeSet::new();
            for (&neighbor, keeps) in &retained {
                if keeps.len() == 1 && queued.insert(neighbor) {
                    forced.push((neighbor, keeps[0]));
                }
            }
            if propagate_forced(&mut retained, forced, queued, conflicts, work)?
                || clique_bound_violated(&retained, conflicts)
            {
                return Ok(None);
            }
            Ok(Some(retained))
        }

        fn strands_neighbor(
            domain_index: usize,
            view: RegisterViewId,
            residual: &BTreeMap<FixedPrecoloredHomeDomainId, Vec<RegisterViewId>>,
            domains: &[Domain],
            conflicts: &Conflicts,
            work: &mut Work,
        ) -> Result<bool, FixedPrecoloredSegmentHomeError> {
            let mut retained = residual.clone();
            let home = (domains[domain_index].id, view);
            Ok(propagate_forced(
                &mut retained,
                vec![home],
                BTreeSet::from([home.0]),
                conflicts,
                work,
            )? || clique_bound_violated(&retained, conflicts))
        }

        fn propagate_forced(
            retained: &mut BTreeMap<FixedPrecoloredHomeDomainId, Vec<RegisterViewId>>,
            mut forced: Vec<(FixedPrecoloredHomeDomainId, RegisterViewId)>,
            mut queued: BTreeSet<FixedPrecoloredHomeDomainId>,
            conflicts: &Conflicts,
            work: &mut Work,
        ) -> Result<bool, FixedPrecoloredSegmentHomeError> {
            let mut cursor = 0;
            while cursor < forced.len() {
                let (member, home) = forced[cursor];
                cursor += 1;
                work.validation_steps(1)?;
                work.iterations(1)?;
                for &(neighbor, conflicting_view) in conflicts.conflicting(member, home) {
                    work.candidates(1)?;
                    work.validation_steps(1)?;
                    work.iterations(1)?;
                    let Some(keeps) = retained.get_mut(&neighbor) else {
                        continue;
                    };
                    if let Some(position) = keeps
                        .iter()
                        .position(|&candidate| candidate == conflicting_view)
                    {
                        keeps.remove(position);
                    }
                    match keeps.len() {
                        0 => return Ok(true),
                        1 if queued.insert(neighbor) => forced.push((neighbor, keeps[0])),
                        _ => {}
                    }
                }
            }
            Ok(false)
        }

        fn clique_bound_violated(
            retained: &BTreeMap<FixedPrecoloredHomeDomainId, Vec<RegisterViewId>>,
            conflicts: &Conflicts,
        ) -> bool {
            for (&base, pool) in retained {
                if pool.len() < 2 {
                    continue;
                }
                let mut clique = vec![base];
                for (&neighbor, keeps) in retained {
                    if neighbor == base || keeps.len() > pool.len() {
                        continue;
                    }
                    if !keeps.iter().all(|candidate| pool.contains(candidate)) {
                        continue;
                    }
                    if clique
                        .iter()
                        .all(|&member| conflicts.domains(member, neighbor))
                    {
                        clique.push(neighbor);
                        if clique.len() > pool.len() {
                            return true;
                        }
                    }
                }
            }
            false
        }
    }
    mod roots {
        use register_model::{
            PreparedRegisterConstraintCatalog, TargetRegisterEnvironmentConstraintKeys,
            TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
            ValidatedRegisterReservationProfile, infrastructure_environment_agrees,
        };

        use crate::{
            FixedPrecoloredSegmentHomeError, ValidatedAllocationLegality,
            ValidatedFixedPrecoloredIntervals, ValidatedFixedPrecoloredSplitRequirements,
            ValidatedLiveRanges,
        };
        use register_homes::{
            FixedPrecoloredIntervalPolicy, FixedPrecoloredSplitRequirementPolicy,
        };

        #[allow(clippy::too_many_arguments)]
        pub(super) fn validate(
            ranges: &ValidatedLiveRanges,
            legality: &ValidatedAllocationLegality,
            fixed: &ValidatedFixedPrecoloredIntervals,
            requirements: &ValidatedFixedPrecoloredSplitRequirements,
            register_environment: TargetRegisterEnvironmentIdentity,
            physical: &ValidatedPhysicalRegisterModel,
            constraints: &PreparedRegisterConstraintCatalog,
            reservations: &ValidatedRegisterReservationProfile,
            selected_keys: &TargetRegisterEnvironmentConstraintKeys,
        ) -> Result<(), FixedPrecoloredSegmentHomeError> {
            if requirements.receipt().fixed_intervals() != fixed.receipt().identity()
                || requirements.receipt().ranges() != ranges.receipt().identity()
                || requirements.receipt().legality() != legality.receipt().identity()
                || requirements.receipt().register_environment() != register_environment
                || requirements.receipt().allocator_availability()
                    != legality.receipt().allocator_availability()
                || requirements.receipt().optimization_unit()
                    != ranges.receipt().optimization_unit()
                || requirements.receipt().work_schedule() != ranges.receipt().work_schedule()
                || requirements.receipt().target() != ranges.plan().target
                || requirements.receipt().policy()
                    != FixedPrecoloredSplitRequirementPolicy::FixedUseBoundaryRequirementsV1
                || fixed.receipt().policy()
                    != FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1
                || legality.receipt().ranges() != ranges.receipt().identity()
                || !infrastructure_environment_agrees(
                    ranges.plan().target,
                    physical,
                    constraints,
                    reservations,
                    selected_keys,
                    register_environment,
                )
            {
                return Err(FixedPrecoloredSegmentHomeError::RootMismatch);
            }
            Ok(())
        }
    }

    use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
    use register_model::{
        PreparedRegisterConstraintCatalog, TargetRegisterEnvironmentConstraintKeys,
        TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
        ValidatedRegisterReservationProfile,
    };

    use crate::work_accounting::Work;

    use crate::{
        FixedPrecoloredSegmentHomeError, ValidatedAllocationLegality,
        ValidatedFixedPrecoloredIntervals, ValidatedFixedPrecoloredSplitRequirements,
        ValidatedLiveRanges,
    };
    use register_homes::{FixedPrecoloredSegmentHomePlan, FixedPrecoloredSegmentHomePolicy};

    #[allow(clippy::too_many_arguments)]
    pub fn compute(
        ranges: &ValidatedLiveRanges,
        legality: &ValidatedAllocationLegality,
        fixed: &ValidatedFixedPrecoloredIntervals,
        requirements: &ValidatedFixedPrecoloredSplitRequirements,
        register_environment: TargetRegisterEnvironmentIdentity,
        physical: &ValidatedPhysicalRegisterModel,
        constraints: &PreparedRegisterConstraintCatalog,
        reservations: &ValidatedRegisterReservationProfile,
        selected_keys: &TargetRegisterEnvironmentConstraintKeys,
        policy: FixedPrecoloredSegmentHomePolicy,
        budget: OptimizationWorkBudget,
    ) -> Result<FixedPrecoloredSegmentHomePlan, FixedPrecoloredSegmentHomeError> {
        roots::validate(
            ranges,
            legality,
            fixed,
            requirements,
            register_environment,
            physical,
            constraints,
            reservations,
            selected_keys,
        )?;
        match policy {
            FixedPrecoloredSegmentHomePolicy::MostConstrainedLowestCompatibleViewV1 => {}
        }
        let mut work = Work::new(OptimizationWorkUsage {
            rule_evaluations: 1,
            validation_steps: 1,
            commits: 1,
            iterations: 1,
            ..Default::default()
        });
        let functions = functions::derive(
            &requirements.plan().functions,
            &ranges.plan().functions,
            physical,
            &mut work,
        )?;
        let usage = work.finish(budget)?;
        Ok(FixedPrecoloredSegmentHomePlan {
            split_requirements: requirements.receipt().identity(),
            fixed_intervals: fixed.receipt().identity(),
            ranges: ranges.receipt().identity(),
            legality: legality.receipt().identity(),
            register_environment,
            allocator_availability: legality.receipt().allocator_availability(),
            optimization_unit: ranges.receipt().optimization_unit(),
            work_schedule: ranges.receipt().work_schedule(),
            target: ranges.plan().target,
            policy,
            budget,
            usage,
            functions,
        })
    }
}
mod domains {
    use std::collections::BTreeMap;

    use register_model::{RegisterClassId, RegisterViewId};
    use selected_instructions::{SelectedBlockId, VirtualRegisterId};

    use crate::FixedPrecoloredSegmentHomeError;
    use register_homes::{
        FixedPrecoloredHomeDomainId, FixedPrecoloredSourceSegmentId,
        FixedPrecoloredSourceSegmentOpening, FunctionFixedPrecoloredSplitRequirements,
    };
    use selected_instructions::LiveRangePoint;

    use crate::work_accounting::Work;

    #[derive(Debug, Clone)]
    pub(super) struct Segment {
        pub(super) id: FixedPrecoloredSourceSegmentId,
        pub(super) block: SelectedBlockId,
        pub(super) start: LiveRangePoint,
        pub(super) end: LiveRangePoint,
    }

    #[derive(Debug, Clone)]
    pub(super) struct Domain {
        pub(super) id: FixedPrecoloredHomeDomainId,
        pub(super) virtual_register: VirtualRegisterId,
        pub(super) class: RegisterClassId,
        pub(super) segments: Vec<Segment>,
        pub(super) candidates: Vec<RegisterViewId>,
    }

    impl Domain {
        pub(super) fn first_point(&self) -> LiveRangePoint {
            self.segments
                .iter()
                .map(|segment| segment.start)
                .min()
                .expect("domain owns a source segment")
        }

        pub(super) fn first_segment(&self) -> FixedPrecoloredSourceSegmentId {
            self.segments
                .iter()
                .map(|segment| segment.id)
                .min()
                .expect("domain owns a source segment")
        }
    }

    pub(super) fn build(
        function: usize,
        requirements: &FunctionFixedPrecoloredSplitRequirements,
        work: &mut Work,
    ) -> Result<Vec<Domain>, FixedPrecoloredSegmentHomeError> {
        work.rule_evaluations(1)?;
        work.validation_steps(1)?;
        work.iterations(1)?;
        let mut domains = Vec::<Domain>::new();
        for register in &requirements.registers {
            work.rule_evaluations(1)?;
            work.validation_steps(1)?;
            work.iterations(1)?;
            let mut closing_by_block = BTreeMap::<SelectedBlockId, usize>::new();
            for fragment in &register.fragments {
                let mut current = None;
                for segment in &fragment.segments {
                    work.validation_steps(1)?;
                    work.commits(1)?;
                    work.iterations(1)?;
                    let domain = match segment.opening {
                        FixedPrecoloredSourceSegmentOpening::SourceRangeStartV1 => new_domain(
                            function,
                            register.virtual_register,
                            register.class,
                            segment.candidates.clone(),
                            &mut domains,
                            work,
                        )?,
                        FixedPrecoloredSourceSegmentOpening::IncomingSourceEdgeV1 { connector } => {
                            let domain = closing_by_block.get(&connector.source).copied().ok_or(
                                FixedPrecoloredSegmentHomeError::MissingIncomingDomain {
                                    function,
                                    register: register.virtual_register.0,
                                    block: connector.source.0,
                                },
                            )?;
                            intersect(
                                function,
                                register.virtual_register,
                                segment.id,
                                &mut domains[domain].candidates,
                                &segment.candidates,
                            )?;
                            domain
                        }
                        FixedPrecoloredSourceSegmentOpening::IncompatibleFixedUseDomainBoundaryV1 {
                            ..
                        } => new_domain(
                            function,
                            register.virtual_register,
                            register.class,
                            segment.candidates.clone(),
                            &mut domains,
                            work,
                        )?,
                    };
                    domains[domain].segments.push(Segment {
                        id: segment.id,
                        block: fragment.block,
                        start: segment.start,
                        end: segment.end,
                    });
                    current = Some(domain);
                }
                let current = current.ok_or(FixedPrecoloredSegmentHomeError::SegmentMismatch {
                    function,
                    register: register.virtual_register.0,
                    segment: 0,
                })?;
                closing_by_block.insert(fragment.block, current);
            }
        }
        Ok(domains)
    }

    fn new_domain(
        function: usize,
        virtual_register: VirtualRegisterId,
        class: RegisterClassId,
        candidates: Vec<RegisterViewId>,
        domains: &mut Vec<Domain>,
        work: &mut Work,
    ) -> Result<usize, FixedPrecoloredSegmentHomeError> {
        let raw = u32::try_from(domains.len())
            .map_err(|_| FixedPrecoloredSegmentHomeError::DomainIdentityOverflow { function })?;
        if candidates.is_empty() {
            return Err(FixedPrecoloredSegmentHomeError::EmptyDomain {
                function,
                register: virtual_register.0,
                segment: 0,
            });
        }
        work.rule_evaluations(1)?;
        work.validation_steps(1)?;
        work.commits(1)?;
        work.iterations(1)?;
        domains.push(Domain {
            id: FixedPrecoloredHomeDomainId(raw),
            virtual_register,
            class,
            segments: Vec::new(),
            candidates,
        });
        Ok(domains.len() - 1)
    }

    fn intersect(
        function: usize,
        register: VirtualRegisterId,
        segment: FixedPrecoloredSourceSegmentId,
        current: &mut Vec<RegisterViewId>,
        incoming: &[RegisterViewId],
    ) -> Result<(), FixedPrecoloredSegmentHomeError> {
        current.retain(|candidate| incoming.binary_search(candidate).is_ok());
        if current.is_empty() {
            return Err(FixedPrecoloredSegmentHomeError::EmptyDomain {
                function,
                register: register.0,
                segment: segment.0,
            });
        }
        Ok(())
    }
}
mod error {
    use crate::WorkDefect;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum FixedPrecoloredSegmentHomeError {
        RootMismatch,
        FunctionMismatch {
            function: usize,
        },
        SegmentMismatch {
            function: usize,
            register: u32,
            segment: u32,
        },
        MissingIncomingDomain {
            function: usize,
            register: u32,
            block: u32,
        },
        EmptyDomain {
            function: usize,
            register: u32,
            segment: u32,
        },
        UnknownOrIncompatibleView {
            function: usize,
            register: u32,
            view: u16,
        },
        DomainIdentityOverflow {
            function: usize,
        },
        SegmentPressure {
            function: usize,
            register: u32,
            segment: u32,
        },
        Work(WorkDefect),
        UsageMismatch,
        NonCanonicalFunctions,
    }

    failed_display!(
        FixedPrecoloredSegmentHomeError,
        "fixed/precolored segment-home assignment failed"
    );
}
use domains::Domain;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    CopyAffinity, FunctionLiveRanges, LiveRangePoint, SelectedBlockId, VirtualLiveRange,
    VirtualRegisterId,
};
use std::collections::BTreeMap;

use crate::work_accounting::Work;

pub use error::FixedPrecoloredSegmentHomeError;
use register_homes::FixedPrecoloredSegmentHomePolicy;

use optimization_core::{OptimizationUnitIdentity, OptimizationWorkUsage};
use register_homes::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedPrecoloredIntervalPlanIdentity,
    FixedPrecoloredSegmentHomePlan, FixedPrecoloredSegmentHomePlanIdentity,
    FixedPrecoloredSplitRequirementPlanIdentity,
};
use register_model::{
    PreparedRegisterConstraintCatalog, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterReservationProfile,
};
use selected_instructions::LiveRangeIdentity;
use semantic_vocabulary::WorkScheduleIdentity;
use target::NativeTarget;

#[allow(clippy::too_many_arguments)]
pub fn assign_fixed_precolored_segment_homes(
    ranges: &crate::ValidatedLiveRanges,
    legality: &crate::ValidatedAllocationLegality,
    fixed: &crate::ValidatedFixedPrecoloredIntervals,
    requirements: &crate::ValidatedFixedPrecoloredSplitRequirements,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &PreparedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    policy: FixedPrecoloredSegmentHomePolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedFixedPrecoloredSegmentHomes, FixedPrecoloredSegmentHomeError> {
    let plan = compute::compute(
        ranges,
        legality,
        fixed,
        requirements,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        policy,
        budget,
    )?;
    seal_fixed_precolored_segment_homes(plan)
}

seal_plan!(
    seal_fixed_precolored_segment_homes(FixedPrecoloredSegmentHomePlan)
        -> ValidatedFixedPrecoloredSegmentHomes: FixedPrecoloredSegmentHomeValidationReceipt
        = register_homes::fixed_precolored_segment_home_plan_identity,
        FixedPrecoloredSegmentHomeError
    {
        split_requirements,
        fixed_intervals,
        ranges,
        legality,
        register_environment,
        allocator_availability,
        optimization_unit,
        work_schedule,
        target,
        policy,
        usage
    }
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedPrecoloredSegmentHomeValidationReceipt {
    pub(crate) identity: FixedPrecoloredSegmentHomePlanIdentity,
    pub(crate) split_requirements: FixedPrecoloredSplitRequirementPlanIdentity,
    pub(crate) fixed_intervals: FixedPrecoloredIntervalPlanIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) work_schedule: WorkScheduleIdentity,
    pub(crate) target: NativeTarget,
    pub(crate) policy: FixedPrecoloredSegmentHomePolicy,
    pub(crate) usage: OptimizationWorkUsage,
}

impl FixedPrecoloredSegmentHomeValidationReceipt {
    receipt_accessor_rows!(FixedPrecoloredSegmentHomeValidationReceipt {
        identity: FixedPrecoloredSegmentHomePlanIdentity,
        split_requirements: FixedPrecoloredSplitRequirementPlanIdentity,
        fixed_intervals: FixedPrecoloredIntervalPlanIdentity,
        target: NativeTarget,
        policy: FixedPrecoloredSegmentHomePolicy,
        usage: OptimizationWorkUsage,
    });
    shared_analysis_identity_accessors!(pub);
}

validated_plan_carrier!(
    ValidatedFixedPrecoloredSegmentHomes: FixedPrecoloredSegmentHomePlan,
    FixedPrecoloredSegmentHomeValidationReceipt
);

fn affinity_edges(
    domains: &[Domain],
    ranges: &FunctionLiveRanges,
    work: &mut Work,
) -> Result<BTreeMap<usize, Vec<(usize, usize)>>, FixedPrecoloredSegmentHomeError> {
    let mut members = BTreeMap::<VirtualRegisterId, Vec<usize>>::new();
    for (index, domain) in domains.iter().enumerate() {
        members
            .entry(domain.virtual_register)
            .or_default()
            .push(index);
    }
    let mut by_register = BTreeMap::<VirtualRegisterId, &VirtualLiveRange>::new();
    for range in &ranges.virtual_registers {
        by_register.insert(range.virtual_register, range);
    }
    let mut edges = Vec::new();
    for affinity in &ranges.copy_affinities {
        work.validation_steps(1)?;
        work.iterations(1)?;
        let (Some(source), Some(destination)) = (
            by_register.get(&affinity.source).copied(),
            by_register.get(&affinity.destination).copied(),
        ) else {
            continue;
        };
        let (Some(source_point), Some(destination_point)) = (
            occurrence_point(source, affinity, RegisterOperandAccess::Use),
            occurrence_point(destination, affinity, RegisterOperandAccess::Def),
        ) else {
            continue;
        };
        let (Some(source_domain), Some(destination_domain)) = (
            members.get(&affinity.source).and_then(|member| {
                containing_domain(member, domains, affinity.block, source_point)
            }),
            members.get(&affinity.destination).and_then(|member| {
                containing_domain(member, domains, affinity.block, destination_point)
            }),
        ) else {
            continue;
        };
        if source_domain != destination_domain {
            edges.push((
                source_domain.min(destination_domain),
                source_domain.max(destination_domain),
            ));
        }
    }
    edges.sort_unstable();
    let mut incident = BTreeMap::<usize, Vec<(usize, usize)>>::new();
    for &edge in &edges {
        incident.entry(edge.0).or_default().push(edge);
        incident.entry(edge.1).or_default().push(edge);
    }
    Ok(incident)
}

fn occurrence_point(
    range: &VirtualLiveRange,
    affinity: &CopyAffinity,
    access: RegisterOperandAccess,
) -> Option<LiveRangePoint> {
    range
        .occurrences
        .iter()
        .find(|occurrence| {
            occurrence.instruction == affinity.instruction && occurrence.access == access
        })
        .map(|occurrence| occurrence.point)
}

fn containing_domain(
    members: &[usize],
    domains: &[Domain],
    block: SelectedBlockId,
    point: LiveRangePoint,
) -> Option<usize> {
    members.iter().copied().find(|&index| {
        domains[index]
            .segments
            .iter()
            .any(|segment| segment.block == block && segment.start <= point && point < segment.end)
    })
}

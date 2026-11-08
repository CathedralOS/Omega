/*
We solve transition-free, spill-free virtual-to-physical register home
assignment and independently replay the resulting `RegisterHomePlan` against
live ranges, allocation legality, tied-operand components, directional early
clobbers, and target register-unit interference.
*/

use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instruction_analyses::{ValidatedAllocationLegality, ValidatedLiveRanges};
pub(crate) mod compute {
    pub(crate) mod conflicts {
        use selected_instructions::VirtualRegisterId;

        use selected_instructions::{VirtualInterference, VirtualInterferences};

        pub(crate) fn registers_interfere(
            left: VirtualRegisterId,
            right: VirtualRegisterId,
            interference: &VirtualInterferences,
        ) -> bool {
            let (lower, higher) = if left < right {
                (left, right)
            } else {
                (right, left)
            };
            interference
                .binary_search(&VirtualInterference { lower, higher })
                .is_ok()
        }
    }
    pub(crate) mod domain {
        use register_model::RegisterViewId;
        use selected_instructions::VirtualRegisterId;

        use super::conflicts::registers_interfere;
        use crate::RegisterHomeError;
        use register_homes::{FunctionAllocationLegality, VirtualRegisterAllocationLegality};
        use selected_instructions::{FunctionLiveRanges, LiveRangePoint};

        #[derive(Debug)]
        pub(crate) struct AllocationDomain<'a> {
            pub(crate) leader_member: &'a VirtualRegisterAllocationLegality,
            pub(crate) extra_members: Vec<&'a VirtualRegisterAllocationLegality>,
            pub(crate) first_point: LiveRangePoint,
            pub(crate) candidates_mask: u128,
            pub(crate) candidates: Vec<RegisterViewId>,
        }

        impl<'a> AllocationDomain<'a> {
            #[inline]
            pub(crate) fn leader(&self) -> VirtualRegisterId {
                self.leader_member.virtual_register
            }

            pub(crate) fn materialize_candidates(&self) -> Vec<RegisterViewId> {
                if !self.candidates.is_empty() {
                    return self.candidates.clone();
                }
                let mut out = Vec::with_capacity(self.candidates_mask.count_ones() as usize);
                let mut mask = self.candidates_mask;
                while mask != 0 {
                    let bit = mask.trailing_zeros() as u16;
                    mask &= mask - 1;
                    out.push(RegisterViewId(bit));
                }
                out
            }
        }

        struct DomainCandidates {
            mask: u128,
            overflow: Vec<RegisterViewId>,
        }

        impl DomainCandidates {
            #[inline]
            fn is_empty(&self) -> bool {
                self.mask == 0 && self.overflow.is_empty()
            }
        }

        pub(crate) fn build_domains<'a>(
            function: usize,
            legality: &'a FunctionAllocationLegality,
            ranges: &FunctionLiveRanges,
        ) -> Result<Vec<AllocationDomain<'a>>, RegisterHomeError> {
            let mut referenced_cache = None;
            for (register, range) in legality
                .virtual_registers
                .iter()
                .zip(&ranges.virtual_registers)
            {
                super::super::physical_requirement::validate(
                    function,
                    register,
                    range,
                    ranges,
                    &mut referenced_cache,
                )?;
            }
            if ranges.tied_pairs.is_empty() && ranges.edge_transfers.is_empty() {
                let mut domains = Vec::with_capacity(legality.virtual_registers.len());
                for register in &legality.virtual_registers {
                    if register.spans.is_empty() {
                        continue;
                    }
                    if !register.entry_transitions.is_empty() {
                        return Err(RegisterHomeError::UnresolvedEntryTransitions {
                            function,
                            register: register.virtual_register.0,
                            count: register.entry_transitions.len(),
                        });
                    }
                    let (first_point, _) = interval_bounds(function, register)?;
                    let cands = common_candidates_repr(function, register)?;
                    domains.push(AllocationDomain {
                        leader_member: register,
                        extra_members: Vec::new(),
                        first_point,
                        candidates_mask: cands.mask,
                        candidates: cands.overflow,
                    });
                }
                return Ok(domains);
            }
            tied_components(function, legality, ranges)?
                .into_iter()
                .filter(|(leader, extra)| {
                    !leader.spans.is_empty() || extra.iter().any(|member| !member.spans.is_empty())
                })
                .map(|(mut leader, mut extra)| {
                    match build_domain(function, &mut leader, &mut extra) {
                        Ok((first_point, cands)) => Ok(AllocationDomain {
                            leader_member: leader,
                            extra_members: extra,
                            first_point,
                            candidates_mask: cands.mask,
                            candidates: cands.overflow,
                        }),
                        Err(error) => {
                            let edge = || {
                                ranges.edge_transfers.iter().find(|edge| {
                                    leader.virtual_register == edge.parameter
                                        || extra
                                            .iter()
                                            .any(|member| member.virtual_register == edge.parameter)
                                })
                            };
                            if matches!(error, RegisterHomeError::NoCommonTiedComponent { .. })
                                && let Some(edge) = edge()
                            {
                                Err(RegisterHomeError::UnsupportedEdgeTransfer {
                                    function,
                                    edge: edge.psi_edge.get(),
                                })
                            } else {
                                Err(error)
                            }
                        }
                    }
                })
                .collect()
        }

        fn tied_components<'a>(
            function: usize,
            legality: &'a FunctionAllocationLegality,
            ranges: &FunctionLiveRanges,
        ) -> Result<
            Vec<(
                &'a VirtualRegisterAllocationLegality,
                Vec<&'a VirtualRegisterAllocationLegality>,
            )>,
            RegisterHomeError,
        > {
            let max_reg_id = legality
                .virtual_registers
                .iter()
                .map(|register| register.virtual_register.0 as usize)
                .max()
                .unwrap_or(0);
            let mut positions = vec![u32::MAX; max_reg_id + 1];
            for (position, register) in legality.virtual_registers.iter().enumerate() {
                positions[register.virtual_register.0 as usize] = position as u32;
            }
            let get_pos = |id: VirtualRegisterId| -> Option<usize> {
                positions
                    .get(id.0 as usize)
                    .copied()
                    .filter(|&pos| pos != u32::MAX)
                    .map(|pos| pos as usize)
            };
            for tie in &ranges.tied_pairs {
                let (Some(used), Some(defined)) = (
                    get_pos(tie.use_virtual_register),
                    get_pos(tie.def_virtual_register),
                ) else {
                    return Err(unsupported_tie(function, tie.instruction.0));
                };
                if used == defined
                    || legality.virtual_registers[used].class != tie.class
                    || legality.virtual_registers[defined].class != tie.class
                {
                    return Err(unsupported_tie(function, tie.instruction.0));
                }
            }
            for transfer in &ranges.edge_transfers {
                let error = || RegisterHomeError::UnsupportedEdgeTransfer {
                    function,
                    edge: transfer.psi_edge.get(),
                };
                let (Some(argument), Some(parameter)) =
                    (get_pos(transfer.argument), get_pos(transfer.parameter))
                else {
                    return Err(error());
                };
                if legality.virtual_registers[argument].class != transfer.class
                    || legality.virtual_registers[parameter].class != transfer.class
                {
                    return Err(error());
                }
            }
            let components = crate::assignment::same_home::SameHomeComponents::from_ranges(ranges);
            let mut grouped: Vec<
                Option<(
                    &'a VirtualRegisterAllocationLegality,
                    Vec<&'a VirtualRegisterAllocationLegality>,
                )>,
            > = vec![None; legality.virtual_registers.len()];
            for (pos, register) in legality.virtual_registers.iter().enumerate() {
                let leader = components.leader(register.virtual_register);
                let leader_pos = if leader == register.virtual_register {
                    pos
                } else {
                    get_pos(leader).expect("component leader is a declared virtual register")
                };
                if let Some((_, extra)) = &mut grouped[leader_pos] {
                    extra.push(register);
                } else {
                    grouped[leader_pos] = Some((register, Vec::new()));
                }
            }
            let grouped: Vec<_> = grouped.into_iter().flatten().collect();
            let mut clobbered_leaders = Vec::new();
            for early in &ranges.early_clobbers {
                let def_leader = components.leader(early.def_virtual_register);
                if early
                    .uses
                    .iter()
                    .any(|used| components.leader(used.virtual_register) == def_leader)
                {
                    clobbered_leaders.push(def_leader);
                }
            }
            if !clobbered_leaders.is_empty() {
                clobbered_leaders.sort_unstable();
                clobbered_leaders.dedup();
            }
            for (leader, extra) in &grouped {
                if !extra.is_empty() {
                    let has_member = |reg: VirtualRegisterId| {
                        leader.virtual_register == reg
                            || extra.iter().any(|member| member.virtual_register == reg)
                    };
                    let member_leader = components.leader(leader.virtual_register);
                    if clobbered_leaders.binary_search(&member_leader).is_ok()
                        && ranges.early_clobbers.iter().any(|early| {
                            has_member(early.def_virtual_register)
                                && early
                                    .uses
                                    .iter()
                                    .any(|used| has_member(used.virtual_register))
                        })
                        && let Some(edge) = ranges
                            .edge_transfers
                            .iter()
                            .find(|edge| has_member(edge.parameter))
                    {
                        return Err(RegisterHomeError::UnsupportedEdgeTransfer {
                            function,
                            edge: edge.psi_edge.get(),
                        });
                    }
                    let reject = |left: VirtualRegisterId, right: VirtualRegisterId| {
                        if !registers_interfere(left, right, &ranges.interference) {
                            return Ok(());
                        }
                        let (lower, higher) = ordered_pair(left, right);
                        if let Some(edge) = ranges
                            .edge_transfers
                            .iter()
                            .find(|edge| has_member(edge.parameter))
                        {
                            return Err(RegisterHomeError::UnsupportedEdgeTransfer {
                                function,
                                edge: edge.psi_edge.get(),
                            });
                        }
                        Err(RegisterHomeError::TiedRegistersInterfere {
                            function,
                            lower: lower.0,
                            higher: higher.0,
                        })
                    };
                    for right in extra {
                        reject(leader.virtual_register, right.virtual_register)?;
                    }
                    for (index, left) in extra.iter().enumerate() {
                        for right in extra.iter().skip(index + 1) {
                            reject(left.virtual_register, right.virtual_register)?;
                        }
                    }
                }
            }
            Ok(grouped)
        }

        fn build_domain<'a>(
            function: usize,
            leader: &mut &'a VirtualRegisterAllocationLegality,
            extra: &mut Vec<&'a VirtualRegisterAllocationLegality>,
        ) -> Result<(LiveRangePoint, DomainCandidates), RegisterHomeError> {
            if !extra.is_empty() {
                extra.push(*leader);
                extra.sort_by_key(|member| member.virtual_register);
                *leader = extra.remove(0);
            }
            let mut first_point = None;
            let mut shared_mask = u128::MAX;
            let mut shared_vec = None::<Vec<RegisterViewId>>;
            for member in std::iter::once(*leader).chain(extra.iter().copied()) {
                if !member.entry_transitions.is_empty() {
                    return Err(RegisterHomeError::UnresolvedEntryTransitions {
                        function,
                        register: member.virtual_register.0,
                        count: member.entry_transitions.len(),
                    });
                }
                let (first, _) = interval_bounds(function, member)?;
                first_point =
                    Some(first_point.map_or(first, |point: LiveRangePoint| point.min(first)));
                let member_cands = common_candidates_repr(function, member)?;
                if shared_vec.is_none() && member_cands.overflow.is_empty() {
                    shared_mask &= member_cands.mask;
                } else {
                    let member_vec = if member_cands.overflow.is_empty() {
                        mask_to_vec(member_cands.mask)
                    } else {
                        member_cands.overflow
                    };
                    if let Some(shared) = &mut shared_vec {
                        shared.retain(|candidate| member_vec.binary_search(candidate).is_ok());
                    } else if shared_mask == u128::MAX {
                        shared_vec = Some(member_vec);
                    } else {
                        let mut vec = mask_to_vec(shared_mask);
                        vec.retain(|candidate| member_vec.binary_search(candidate).is_ok());
                        shared_vec = Some(vec);
                    }
                }
            }
            let cands = if let Some(vec) = shared_vec {
                if vec.iter().all(|v| (v.0 as usize) < 128) {
                    let mut mask = 0u128;
                    for &v in &vec {
                        mask |= 1u128 << v.0;
                    }
                    DomainCandidates {
                        mask,
                        overflow: Vec::new(),
                    }
                } else {
                    DomainCandidates {
                        mask: 0,
                        overflow: vec,
                    }
                }
            } else {
                DomainCandidates {
                    mask: shared_mask,
                    overflow: Vec::new(),
                }
            };
            if cands.is_empty() {
                if !extra.is_empty() {
                    return Err(RegisterHomeError::NoCommonTiedComponent {
                        function,
                        leader: leader.virtual_register.0,
                        member_count: 1 + extra.len(),
                    });
                }
                return Err(RegisterHomeError::NoCommonCandidate {
                    function,
                    register: leader.virtual_register.0,
                });
            }
            Ok((first_point.expect("allocation domain is nonempty"), cands))
        }

        fn mask_to_vec(mut mask: u128) -> Vec<RegisterViewId> {
            let mut out = Vec::with_capacity(mask.count_ones() as usize);
            while mask != 0 {
                let bit = mask.trailing_zeros() as u16;
                mask &= mask - 1;
                out.push(RegisterViewId(bit));
            }
            out
        }

        fn first_span(
            function: usize,
            register: &VirtualRegisterAllocationLegality,
        ) -> Result<&register_homes::VirtualLegalitySpan, RegisterHomeError> {
            register
                .spans
                .first()
                .ok_or(RegisterHomeError::NoLivePoints {
                    function,
                    register: register.virtual_register.0,
                })
        }

        fn interval_bounds(
            function: usize,
            register: &VirtualRegisterAllocationLegality,
        ) -> Result<(LiveRangePoint, LiveRangePoint), RegisterHomeError> {
            let first = first_span(function, register)?;
            let last = register.spans.last().expect("nonempty spans established");
            Ok((first.start, last.end))
        }

        fn common_candidates_repr(
            function: usize,
            register: &VirtualRegisterAllocationLegality,
        ) -> Result<DomainCandidates, RegisterHomeError> {
            let first = first_span(function, register)?;
            if register.spans.len() == 1 && register.early_clobber_points.is_empty() {
                if first.candidates.is_empty() {
                    return Err(RegisterHomeError::NoCommonCandidate {
                        function,
                        register: register.virtual_register.0,
                    });
                }
                let mut mask = 0u128;
                let mut all_under_128 = true;
                for &v in &first.candidates {
                    if (v.0 as usize) < 128 {
                        mask |= 1u128 << v.0;
                    } else {
                        all_under_128 = false;
                        break;
                    }
                }
                if all_under_128 {
                    return Ok(DomainCandidates {
                        mask,
                        overflow: Vec::new(),
                    });
                }
            }
            let common = intersect_candidates_repr(
                &first.candidates,
                register.spans[1..]
                    .iter()
                    .map(|point| point.candidates.as_slice())
                    .chain(
                        register
                            .early_clobber_points
                            .iter()
                            .map(|point| point.candidates.as_slice()),
                    ),
            );
            if common.is_empty() {
                if register
                    .spans
                    .iter()
                    .all(|point| !point.candidates.is_empty())
                    && register
                        .early_clobber_points
                        .iter()
                        .all(|point| !point.candidates.is_empty())
                {
                    return Err(RegisterHomeError::NoCompatibleHome {
                        function,
                        register: register.virtual_register.0,
                    });
                }
                return Err(RegisterHomeError::NoCommonCandidate {
                    function,
                    register: register.virtual_register.0,
                });
            }
            Ok(common)
        }

        fn intersect_candidates_repr<'a>(
            first: &[RegisterViewId],
            rest: impl Iterator<Item = &'a [RegisterViewId]>,
        ) -> DomainCandidates {
            if first.iter().all(|v| (v.0 as usize) < 128) {
                let mut mask = 0u128;
                for &v in first {
                    mask |= 1u128 << v.0;
                }
                let mut fallback = false;
                let mut rest_iter = rest.peekable();
                while let Some(&candidates) = rest_iter.peek() {
                    if mask == 0 {
                        return DomainCandidates {
                            mask: 0,
                            overflow: Vec::new(),
                        };
                    }
                    if !candidates.iter().all(|v| (v.0 as usize) < 128) {
                        fallback = true;
                        break;
                    }
                    rest_iter.next();
                    let mut cm = 0u128;
                    for &v in candidates {
                        cm |= 1u128 << v.0;
                    }
                    mask &= cm;
                }
                if !fallback {
                    return DomainCandidates {
                        mask,
                        overflow: Vec::new(),
                    };
                }
                let mut common = mask_to_vec(mask);
                for candidates in rest_iter {
                    if common.is_empty() {
                        break;
                    }
                    if candidates != common.as_slice() {
                        common.retain(|candidate| candidates.binary_search(candidate).is_ok());
                    }
                }
                return DomainCandidates {
                    mask: 0,
                    overflow: common,
                };
            }
            let mut common = first.to_vec();
            if common.windows(2).any(|pair| pair[0] >= pair[1]) {
                common.sort_unstable();
                common.dedup();
            }
            for candidates in rest {
                if common.is_empty() {
                    break;
                }
                if candidates != common.as_slice() {
                    common.retain(|candidate| candidates.binary_search(candidate).is_ok());
                }
            }
            DomainCandidates {
                mask: 0,
                overflow: common,
            }
        }

        #[cfg(test)]
        fn intersect_candidates<'a>(
            first: &[RegisterViewId],
            rest: impl Iterator<Item = &'a [RegisterViewId]>,
        ) -> Vec<RegisterViewId> {
            let cands = intersect_candidates_repr(first, rest);
            if cands.overflow.is_empty() {
                mask_to_vec(cands.mask)
            } else {
                cands.overflow
            }
        }

        fn ordered_pair(
            left: VirtualRegisterId,
            right: VirtualRegisterId,
        ) -> (VirtualRegisterId, VirtualRegisterId) {
            if left < right {
                (left, right)
            } else {
                (right, left)
            }
        }

        fn unsupported_tie(function: usize, instruction: u32) -> RegisterHomeError {
            RegisterHomeError::UnsupportedTiedTopology {
                function,
                instruction,
            }
        }

        #[cfg(test)]
        mod tests {
            use super::intersect_candidates;
            use register_model::RegisterViewId;
            use std::collections::BTreeSet;

            #[test]
            fn candidate_intersection_matches_set_retain() {
                let mut seed = 0x9e37_79b9_7f4a_7c15u64;
                let mut next = |bound: u64| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    seed % bound
                };
                for _ in 0..300 {
                    let lists = (0..next(6) + 1)
                        .map(|_| {
                            let mut list = (0..next(12))
                                .map(|_| RegisterViewId(next(16) as u16))
                                .collect::<Vec<_>>();
                            list.sort();
                            list.dedup();
                            list
                        })
                        .collect::<Vec<_>>();
                    let mut expected = lists[0].iter().copied().collect::<BTreeSet<_>>();
                    for list in &lists[1..] {
                        expected.retain(|candidate| list.binary_search(candidate).is_ok());
                    }
                    let actual =
                        intersect_candidates(&lists[0], lists[1..].iter().map(Vec::as_slice))
                            .into_iter()
                            .collect::<BTreeSet<_>>();
                    assert_eq!(actual, expected);
                }
            }
        }
    }
    pub(crate) mod placement {
        use std::cmp::Reverse;
        use std::collections::BTreeMap;

        use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
        use selected_instructions::{LiveRangePoint, VirtualRegisterId};

        use super::domain::{AllocationDomain, build_domains};
        use super::prepared_affinities::PreparedAffinities;
        use super::prepared_conflicts::PreparedConflicts;
        use crate::{FunctionRegisterHomes, RegisterHomeError, VirtualRegisterHome};
        use register_homes::FunctionAllocationLegality;
        use selected_instructions::FunctionLiveRanges;

        pub(crate) fn compute_function(
            function: usize,
            legality: &FunctionAllocationLegality,
            ranges: &FunctionLiveRanges,
            physical: &ValidatedPhysicalRegisterModel,
            mut pressure: Option<&mut Vec<RegisterHomeError>>,
        ) -> Result<FunctionRegisterHomes, RegisterHomeError> {
            if legality.virtual_registers.len() != ranges.virtual_registers.len() {
                return Err(RegisterHomeError::FunctionMismatch { function });
            }
            let domains = build_domains(function, legality, ranges)?;
            let max_reg_id = legality
                .virtual_registers
                .iter()
                .map(|register| register.virtual_register.0 as usize)
                .max()
                .unwrap_or(0);
            let mut domain_of = vec![u32::MAX; max_reg_id + 1];
            for (domain_index, domain) in domains.iter().enumerate() {
                domain_of[domain.leader_member.virtual_register.0 as usize] = domain_index as u32;
                for &member in &domain.extra_members {
                    domain_of[member.virtual_register.0 as usize] = domain_index as u32;
                }
            }
            let affinities =
                PreparedAffinities::new(domains.len(), &domain_of, &ranges.copy_affinities);
            let conflicts = PreparedConflicts::new(
                function,
                &domains,
                &domain_of,
                ranges,
                physical,
                &affinities,
            )?;
            if conflicts.uses_bitmasks() {
                return compute_function_bitmask(
                    function,
                    legality,
                    &domains,
                    max_reg_id,
                    &affinities,
                    &conflicts,
                    pressure,
                );
            }
            let mut active = vec![true; domains.len()];
            let mut homes = vec![None::<RegisterViewId>; max_reg_id + 1];
            let mut viable = domains
                .iter()
                .map(AllocationDomain::materialize_candidates)
                .collect::<Vec<_>>();
            let mut degrees = (0..domains.len())
                .map(|domain_index| conflicts.degree(domain_index))
                .collect::<Vec<_>>();
            let initial_keys = (0..domains.len())
                .map(|domain_index| selection_key(domain_index, &viable, &degrees, &domains))
                .collect::<Vec<_>>();
            let mut unassigned = PackedSelectionTree::new(&initial_keys);
            while let Some(domain_index) = unassigned.pop_first() {
                active[domain_index] = false;
                let view = preferred_view(
                    domain_index,
                    &viable,
                    &active,
                    &homes,
                    &affinities,
                    &conflicts,
                );
                let Some(view) = view else {
                    let failure = RegisterHomeError::NoCompatibleHome {
                        function,
                        register: domains[domain_index].leader().0,
                    };
                    let Some(pressure) = pressure.as_deref_mut() else {
                        return Err(failure);
                    };
                    pressure.push(failure);
                    for remaining in conflicts.get_constrained_neighbors(domain_index) {
                        if active[remaining] {
                            degrees[remaining] -= 1;
                            unassigned.set_key(
                                remaining,
                                selection_key(remaining, &viable, &degrees, &domains),
                            );
                        }
                    }
                    continue;
                };
                homes[domains[domain_index].leader_member.virtual_register.0 as usize] = Some(view);
                for &member in &domains[domain_index].extra_members {
                    homes[member.virtual_register.0 as usize] = Some(view);
                }
                for (remaining, pair) in conflicts.get_constrained_neighbors_with_pair(domain_index)
                {
                    if !active[remaining] {
                        continue;
                    }
                    degrees[remaining] -= 1;
                    let rev_pair = pair.reversed();
                    let candidates = &mut viable[remaining];
                    let mut retained = 0;
                    for position in 0..candidates.len() {
                        let candidate = candidates[position];
                        if !conflicts.conflicts_with_pair(rev_pair, candidate, view) {
                            candidates[retained] = candidate;
                            retained += 1;
                        }
                    }
                    candidates.truncate(retained);
                    unassigned.set_key(
                        remaining,
                        selection_key(remaining, &viable, &degrees, &domains),
                    );
                }
            }
            if let Some(first) = pressure.and_then(|pressure| pressure.first()).cloned() {
                return Err(first);
            }
            Ok(assemble_homes(legality, &homes))
        }

        fn assemble_homes(
            legality: &FunctionAllocationLegality,
            homes: &[Option<RegisterViewId>],
        ) -> FunctionRegisterHomes {
            FunctionRegisterHomes {
                machine: legality.machine,
                assignments: legality
                    .virtual_registers
                    .iter()
                    .filter(|register| !register.spans.is_empty())
                    .map(|register| VirtualRegisterHome {
                        virtual_register: register.virtual_register,
                        class: register.class,
                        view: homes[register.virtual_register.0 as usize]
                            .expect("active register has assigned home"),
                    })
                    .collect(),
            }
        }

        fn preferred_view(
            domain_index: usize,
            viable: &[Vec<RegisterViewId>],
            active: &[bool],
            homes: &[Option<RegisterViewId>],
            affinities: &PreparedAffinities,
            conflicts: &PreparedConflicts,
        ) -> Option<RegisterViewId> {
            let candidates = viable[domain_index].as_slice();
            if candidates.is_empty() {
                return None;
            }
            let mut keeping = Vec::with_capacity(candidates.len());
            for &view in candidates {
                if !strands_neighbor(domain_index, view, viable, active, conflicts) {
                    keeping.push(view);
                }
            }
            let pool: &[RegisterViewId] = if keeping.is_empty() {
                candidates
            } else {
                &keeping
            };
            let domain_affinities = affinities.get_domain_affinities(domain_index);
            let outlooks = if domain_affinities.is_empty() {
                BTreeMap::new()
            } else {
                partner_outlooks(domain_index, viable, active, homes, affinities, conflicts)
            };
            let mut stolen_by_pool = vec![0usize; pool.len()];
            for (neighbor, pair) in conflicts.get_affinity_neighbors_with_pair(domain_index) {
                if !active[neighbor] {
                    continue;
                }
                let neighbor_affinities = affinities.get_domain_affinities(neighbor);
                let rev_pair = pair.reversed();
                for affinity in neighbor_affinities {
                    if let Some(view) = homes.get(affinity.partner.0 as usize).copied().flatten()
                        && let Ok(pool_idx) = pool.binary_search(&view)
                        && viable[neighbor].binary_search(&view).is_ok()
                        && conflicts.conflicts_with_pair(rev_pair, view, view)
                    {
                        stolen_by_pool[pool_idx] += 1;
                    }
                }
            }
            let mut leading = None::<(usize, usize, Reverse<usize>, RegisterViewId)>;
            for (pool_idx, &view) in pool.iter().enumerate() {
                let mut guaranteed = 0usize;
                let mut votes = 0usize;
                for affinity in domain_affinities {
                    if homes.get(affinity.partner.0 as usize).copied().flatten() == Some(view) {
                        guaranteed += 1;
                    } else if let Some(partner_domain) = affinity.partner_domain
                        && let Some(outlook) = outlooks.get(&partner_domain)
                        && viable[partner_domain].binary_search(&view).is_ok()
                    {
                        let drawn = outlook.assigned_count(view) + outlook.pending;
                        if drawn > outlook.peak
                            || (drawn == outlook.peak
                                && outlook.peak_view.is_some_and(|peak| view < peak))
                        {
                            votes += 1;
                        }
                    }
                }
                let stolen = stolen_by_pool[pool_idx];
                let rank = (guaranteed, votes, Reverse(stolen));
                if leading.is_none_or(|(best_guaranteed, best_votes, best_stolen, _)| {
                    rank > (best_guaranteed, best_votes, best_stolen)
                }) {
                    leading = Some((guaranteed, votes, Reverse(stolen), view));
                }
            }
            leading
                .map(|(_, _, _, view)| view)
                .or_else(|| pool.first().copied())
        }

        #[derive(Default)]
        struct PartnerOutlook {
            pending: usize,
            assigned: Vec<(RegisterViewId, usize)>,
            peak: usize,
            peak_view: Option<RegisterViewId>,
        }

        impl PartnerOutlook {
            fn assigned_count(&self, view: RegisterViewId) -> usize {
                self.assigned
                    .iter()
                    .find_map(|&(v, count)| (v == view).then_some(count))
                    .unwrap_or(0)
            }

            fn increment_assigned(&mut self, view: RegisterViewId) {
                if let Some((_, count)) = self.assigned.iter_mut().find(|(v, _)| *v == view) {
                    *count += 1;
                } else {
                    self.assigned.push((view, 1));
                }
            }
        }

        fn partner_outlooks(
            domain_index: usize,
            viable: &[Vec<RegisterViewId>],
            active: &[bool],
            homes: &[Option<RegisterViewId>],
            affinities: &PreparedAffinities,
            conflicts: &PreparedConflicts,
        ) -> BTreeMap<usize, PartnerOutlook> {
            let mut outlooks = BTreeMap::<usize, PartnerOutlook>::new();
            for affinity in affinities.get_domain_affinities(domain_index) {
                let Some(partner_domain) = affinity.partner_domain else {
                    continue;
                };
                if partner_domain != domain_index
                    && active[partner_domain]
                    && !conflicts.constrained(partner_domain, domain_index)
                {
                    outlooks.entry(partner_domain).or_default().pending += 1;
                }
            }
            for (&partner_domain, outlook) in &mut outlooks {
                for affinity in affinities.get_domain_affinities(partner_domain) {
                    if let Some(home) = homes.get(affinity.partner.0 as usize).copied().flatten() {
                        outlook.increment_assigned(home);
                    }
                }
            }
            for (partner_domain, outlook) in &mut outlooks {
                for &view in &viable[*partner_domain] {
                    let count = outlook.assigned_count(view);
                    if outlook.peak_view.is_none() || count > outlook.peak {
                        outlook.peak = count;
                        outlook.peak_view = Some(view);
                    }
                }
            }
            outlooks
        }

        fn strands_neighbor(
            domain_index: usize,
            view: RegisterViewId,
            viable: &[Vec<RegisterViewId>],
            active: &[bool],
            conflicts: &PreparedConflicts,
        ) -> bool {
            let max_conflicts = conflicts.max_physical_conflicts(view);
            if max_conflicts == 0 {
                return false;
            }
            for (neighbor, pair) in conflicts.get_constrained_neighbors_with_pair(domain_index) {
                let neighbor_viable = &viable[neighbor];
                if !active[neighbor]
                    || neighbor_viable.is_empty()
                    || neighbor_viable.len() > max_conflicts
                {
                    continue;
                }
                let rev_pair = pair.reversed();
                let mut retains = false;
                for &candidate in neighbor_viable {
                    if !conflicts.conflicts_with_pair(rev_pair, candidate, view) {
                        retains = true;
                        break;
                    }
                }
                if !retains {
                    return true;
                }
            }
            false
        }

        trait SelectionOrder: Copy + Ord {
            const EMPTY: Self;

            fn domain_index(self) -> usize;
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        struct SelectionKey {
            viable: usize,
            degree: Reverse<usize>,
            first_point: LiveRangePoint,
            leader: VirtualRegisterId,
            domain_index: usize,
        }

        impl SelectionOrder for SelectionKey {
            const EMPTY: Self = SelectionKey {
                viable: usize::MAX,
                degree: Reverse(0),
                first_point: LiveRangePoint(0),
                leader: VirtualRegisterId(0),
                domain_index: usize::MAX,
            };

            #[inline]
            fn domain_index(self) -> usize {
                self.domain_index
            }
        }

        fn selection_key(
            domain_index: usize,
            viable: &[Vec<RegisterViewId>],
            degrees: &[usize],
            domains: &[AllocationDomain<'_>],
        ) -> SelectionKey {
            let domain = &domains[domain_index];
            SelectionKey {
                viable: viable[domain_index].len(),
                degree: Reverse(degrees[domain_index]),
                first_point: domain.first_point,
                leader: domain.leader(),
                domain_index,
            }
        }

        trait PackedSelectionKey: SelectionOrder {
            fn pack(viable: u32, degree: usize, static_key: Self) -> Self;
        }

        impl SelectionOrder for u64 {
            const EMPTY: Self = u64::MAX;

            #[inline]
            fn domain_index(self) -> usize {
                (self as u16) as usize
            }
        }

        impl PackedSelectionKey for u64 {
            #[inline]
            fn pack(viable: u32, degree: usize, static_key: Self) -> Self {
                let inv_degree = !(degree as u16);
                ((viable as u64) << 56) | ((inv_degree as u64) << 40) | static_key
            }
        }

        impl SelectionOrder for u128 {
            const EMPTY: Self = u128::MAX;

            #[inline]
            fn domain_index(self) -> usize {
                (self as u32 & 0xFF_FFFF) as usize
            }
        }

        impl PackedSelectionKey for u128 {
            #[inline]
            fn pack(viable: u32, degree: usize, static_key: Self) -> Self {
                let inv_degree = !(degree.min(u32::MAX as usize) as u32);
                ((viable as u128) << 120) | ((inv_degree as u128) << 88) | static_key
            }
        }

        struct PackedSelectionTree<K: SelectionOrder> {
            base: usize,
            tree: Vec<K>,
        }

        impl<K: SelectionOrder> PackedSelectionTree<K> {
            fn new(keys: &[K]) -> Self {
                let n = keys.len();
                let base = n.next_power_of_two().max(1);
                let mut tree = vec![K::EMPTY; 2 * base];
                tree[base..base + n].copy_from_slice(keys);
                for idx in (1..base).rev() {
                    tree[idx] = tree[2 * idx].min(tree[2 * idx + 1]);
                }
                Self { base, tree }
            }

            #[inline]
            fn set_key(&mut self, domain_index: usize, key: K) {
                let mut idx = self.base + domain_index;
                self.tree[idx] = key;
                idx >>= 1;
                while idx > 0 {
                    let winner = self.tree[2 * idx].min(self.tree[2 * idx + 1]);
                    if self.tree[idx] == winner {
                        break;
                    }
                    self.tree[idx] = winner;
                    idx >>= 1;
                }
            }

            #[inline]
            fn pop_first(&mut self) -> Option<usize> {
                let best = self.tree[1];
                if best == K::EMPTY {
                    return None;
                }
                let domain_index = best.domain_index();
                self.set_key(domain_index, K::EMPTY);
                Some(domain_index)
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn compute_function_bitmask(
            function: usize,
            legality: &FunctionAllocationLegality,
            domains: &[AllocationDomain<'_>],
            max_reg_id: usize,
            affinities: &PreparedAffinities,
            conflicts: &PreparedConflicts,
            pressure: Option<&mut Vec<RegisterHomeError>>,
        ) -> Result<FunctionRegisterHomes, RegisterHomeError> {
            let viable_masks = domains
                .iter()
                .map(|domain| domain.candidates_mask)
                .collect::<Vec<_>>();
            let degrees = (0..domains.len())
                .map(|domain_index| conflicts.degree(domain_index))
                .collect::<Vec<_>>();
            if domains.len() <= u16::MAX as usize
                && domains.windows(2).all(|w| w[0].leader() < w[1].leader())
                && domains.iter().all(|d| d.first_point.0 < (1 << 24))
                && degrees.iter().all(|&degree| degree <= u16::MAX as usize)
            {
                let static_keys = domains
                    .iter()
                    .enumerate()
                    .map(|(domain_index, domain)| {
                        ((domain.first_point.0 as u64) << 16) | (domain_index as u64)
                    })
                    .collect::<Vec<_>>();
                place_packed(
                    function,
                    legality,
                    domains,
                    max_reg_id,
                    affinities,
                    conflicts,
                    pressure,
                    viable_masks,
                    degrees,
                    &static_keys,
                )
            } else {
                let static_keys = domains
                    .iter()
                    .enumerate()
                    .map(|(domain_index, domain)| {
                        ((domain.first_point.0 as u128) << 56)
                            | ((domain.leader().0 as u128) << 24)
                            | (domain_index as u128)
                    })
                    .collect::<Vec<_>>();
                place_packed(
                    function,
                    legality,
                    domains,
                    max_reg_id,
                    affinities,
                    conflicts,
                    pressure,
                    viable_masks,
                    degrees,
                    &static_keys,
                )
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn place_packed<K: PackedSelectionKey>(
            function: usize,
            legality: &FunctionAllocationLegality,
            domains: &[AllocationDomain<'_>],
            max_reg_id: usize,
            affinities: &PreparedAffinities,
            conflicts: &PreparedConflicts,
            mut pressure: Option<&mut Vec<RegisterHomeError>>,
            mut viable_masks: Vec<u128>,
            mut degrees: Vec<usize>,
            static_keys: &[K],
        ) -> Result<FunctionRegisterHomes, RegisterHomeError> {
            let mut active = vec![true; domains.len()];
            let mut homes = vec![None::<RegisterViewId>; max_reg_id + 1];
            let mut assigned_partner_mask = if affinities.is_empty() {
                Vec::new()
            } else {
                vec![0u128; domains.len()]
            };
            let initial_keys = (0..domains.len())
                .map(|domain_index| {
                    K::pack(
                        viable_masks[domain_index].count_ones(),
                        degrees[domain_index],
                        static_keys[domain_index],
                    )
                })
                .collect::<Vec<_>>();
            let mut unassigned = PackedSelectionTree::<K>::new(&initial_keys);
            while let Some(domain_index) = unassigned.pop_first() {
                active[domain_index] = false;
                let view = preferred_view_mask(
                    domain_index,
                    &viable_masks,
                    &active,
                    &homes,
                    &assigned_partner_mask,
                    affinities,
                    conflicts,
                );
                let Some(view) = view else {
                    let failure = RegisterHomeError::NoCompatibleHome {
                        function,
                        register: domains[domain_index].leader().0,
                    };
                    let Some(pressure) = pressure.as_deref_mut() else {
                        return Err(failure);
                    };
                    pressure.push(failure);
                    for remaining in conflicts.get_constrained_neighbors(domain_index) {
                        if active[remaining] {
                            degrees[remaining] -= 1;
                            unassigned.set_key(
                                remaining,
                                K::pack(
                                    viable_masks[remaining].count_ones(),
                                    degrees[remaining],
                                    static_keys[remaining],
                                ),
                            );
                        }
                    }
                    continue;
                };
                homes[domains[domain_index].leader_member.virtual_register.0 as usize] = Some(view);
                for &member in &domains[domain_index].extra_members {
                    homes[member.virtual_register.0 as usize] = Some(view);
                }
                if !assigned_partner_mask.is_empty() {
                    let bit = 1u128 << view.0;
                    for affinity in affinities.get_domain_affinities(domain_index) {
                        if let Some(partner_domain) = affinity.partner_domain {
                            assigned_partner_mask[partner_domain] |= bit;
                        }
                    }
                }
                for (remaining, pair) in conflicts.get_constrained_neighbors_with_pair(domain_index)
                {
                    if !active[remaining] {
                        continue;
                    }
                    degrees[remaining] -= 1;
                    let rev_pair = pair.reversed();
                    viable_masks[remaining] &= !conflicts.conflict_mask(rev_pair, view);
                    unassigned.set_key(
                        remaining,
                        K::pack(
                            viable_masks[remaining].count_ones(),
                            degrees[remaining],
                            static_keys[remaining],
                        ),
                    );
                }
            }
            if let Some(first) = pressure.and_then(|pressure| pressure.first()).cloned() {
                return Err(first);
            }
            Ok(assemble_homes(legality, &homes))
        }

        fn preferred_view_mask(
            domain_index: usize,
            viable_masks: &[u128],
            active: &[bool],
            homes: &[Option<RegisterViewId>],
            assigned_partner_mask: &[u128],
            affinities: &PreparedAffinities,
            conflicts: &PreparedConflicts,
        ) -> Option<RegisterViewId> {
            let candidates_mask = viable_masks[domain_index];
            if candidates_mask == 0 {
                return None;
            }
            if candidates_mask.is_power_of_two() {
                return Some(RegisterViewId(candidates_mask.trailing_zeros() as u16));
            }
            let max_conflicts = if conflicts.uniform_physical_conflicts()
                || conflicts.max_any_physical_conflicts() <= 1
            {
                conflicts.max_any_physical_conflicts()
            } else {
                let mut mc = 0usize;
                let mut cm = candidates_mask;
                while cm != 0 {
                    let bit = cm.trailing_zeros() as u16;
                    cm &= cm - 1;
                    mc = mc.max(conflicts.max_physical_conflicts(RegisterViewId(bit)));
                }
                mc
            };
            let mut stranded_mask = 0u128;
            if max_conflicts > 0 && (candidates_mask.count_ones() as usize) <= max_conflicts {
                for (neighbor, pair) in conflicts.get_constrained_neighbors_with_pair(domain_index)
                {
                    if !active[neighbor] {
                        continue;
                    }
                    let n_mask = viable_masks[neighbor];
                    let n_len = n_mask.count_ones() as usize;
                    if n_len == 0 || n_len > max_conflicts {
                        continue;
                    }
                    let mut all_conflict = candidates_mask & !stranded_mask;
                    let mut nm = n_mask;
                    while nm != 0 && all_conflict != 0 {
                        let w_bit = nm.trailing_zeros() as u16;
                        nm &= nm - 1;
                        all_conflict &= conflicts.conflict_mask(pair, RegisterViewId(w_bit));
                    }
                    stranded_mask |= all_conflict;
                    if (candidates_mask & !stranded_mask) == 0 {
                        break;
                    }
                }
            }
            let keeping_mask = candidates_mask & !stranded_mask;
            let pool_mask = if keeping_mask == 0 {
                candidates_mask
            } else {
                keeping_mask
            };
            if affinities.is_empty() || pool_mask.is_power_of_two() {
                return Some(RegisterViewId(pool_mask.trailing_zeros() as u16));
            }
            let domain_affinities = affinities.get_domain_affinities(domain_index);
            let mut search_mask = pool_mask;
            if !domain_affinities.is_empty() {
                let mut guaranteed_mask = 0u128;
                let mut best_guaranteed_mask = 0u128;
                let mut max_guaranteed = 0usize;
                let mut guaranteed_counts = [0u16; 128];
                let mut has_active_partner = false;
                for affinity in domain_affinities {
                    if let Some(view) = homes.get(affinity.partner.0 as usize).copied().flatten() {
                        let bit = 1u128 << view.0;
                        if (pool_mask & bit) != 0 {
                            let idx = usize::from(view.0);
                            let count = if (guaranteed_mask & bit) == 0 {
                                guaranteed_mask |= bit;
                                1usize
                            } else {
                                usize::from(guaranteed_counts[idx]) + 1
                            };
                            guaranteed_counts[idx] = count as u16;
                            if count > max_guaranteed {
                                max_guaranteed = count;
                                best_guaranteed_mask = bit;
                            } else if count == max_guaranteed {
                                best_guaranteed_mask |= bit;
                            }
                        }
                    } else if let Some(partner_domain) = affinity.partner_domain
                        && partner_domain != domain_index
                        && active[partner_domain]
                    {
                        has_active_partner = true;
                    }
                }
                if max_guaranteed > 0 {
                    if best_guaranteed_mask.is_power_of_two() {
                        return Some(RegisterViewId(best_guaranteed_mask.trailing_zeros() as u16));
                    }
                    search_mask = best_guaranteed_mask;
                }
                if has_active_partner {
                    search_mask = narrow_by_partner_votes_mask(
                        domain_index,
                        search_mask,
                        viable_masks,
                        active,
                        homes,
                        affinities,
                        conflicts,
                    );
                    if search_mask.is_power_of_two() {
                        return Some(RegisterViewId(search_mask.trailing_zeros() as u16));
                    }
                }
            }
            if !conflicts.has_affinity_neighbor(domain_index) {
                return Some(RegisterViewId(search_mask.trailing_zeros() as u16));
            }
            let mut stolen_mask = 0u128;
            let mut stolen_by_view = [0u16; 128];
            for (neighbor, pair) in conflicts.get_affinity_neighbors_with_pair(domain_index) {
                if !active[neighbor] {
                    continue;
                }
                let n_mask = viable_masks[neighbor];
                let common_mask = assigned_partner_mask[neighbor] & search_mask & n_mask;
                if common_mask == 0 {
                    continue;
                }
                let neighbor_affinities = affinities.get_domain_affinities(neighbor);
                let rev_pair = pair.reversed();
                for affinity in neighbor_affinities {
                    if let Some(view) = homes.get(affinity.partner.0 as usize).copied().flatten() {
                        let bit = 1u128 << view.0;
                        if (common_mask & bit) != 0
                            && conflicts.conflicts_with_pair(rev_pair, view, view)
                        {
                            let idx = usize::from(view.0);
                            if (stolen_mask & bit) == 0 {
                                stolen_mask |= bit;
                                stolen_by_view[idx] = 1;
                            } else {
                                stolen_by_view[idx] = stolen_by_view[idx].saturating_add(1);
                            }
                        }
                    }
                }
            }
            let unstolen = search_mask & !stolen_mask;
            if unstolen != 0 {
                return Some(RegisterViewId(unstolen.trailing_zeros() as u16));
            }
            let mut best_view = RegisterViewId(search_mask.trailing_zeros() as u16);
            let mut best_stolen = u16::MAX;
            let mut sm = search_mask;
            while sm != 0 {
                let bit = sm.trailing_zeros() as u16;
                sm &= sm - 1;
                let stolen = stolen_by_view[usize::from(bit)];
                if stolen < best_stolen {
                    best_stolen = stolen;
                    best_view = RegisterViewId(bit);
                }
            }
            Some(best_view)
        }

        fn narrow_by_partner_votes_mask(
            domain_index: usize,
            search_mask: u128,
            viable_masks: &[u128],
            active: &[bool],
            homes: &[Option<RegisterViewId>],
            affinities: &PreparedAffinities,
            conflicts: &PreparedConflicts,
        ) -> u128 {
            let domain_affinities = affinities.get_domain_affinities(domain_index);
            let mut partners = Vec::<(usize, usize)>::new();
            for affinity in domain_affinities {
                let Some(partner_domain) = affinity.partner_domain else {
                    continue;
                };
                if partner_domain != domain_index
                    && active[partner_domain]
                    && !conflicts.constrained(partner_domain, domain_index)
                {
                    if let Some((_, pending)) =
                        partners.iter_mut().find(|(d, _)| *d == partner_domain)
                    {
                        *pending += 1;
                    } else {
                        partners.push((partner_domain, 1));
                    }
                }
            }
            if partners.is_empty() {
                return search_mask;
            }
            let single_partner = partners.len() == 1;
            let mut voted_mask = 0u128;
            let mut best_votes_mask = search_mask;
            let mut max_votes = 0usize;
            let mut vote_counts = [0u16; 128];
            let mut assigned = Vec::<(RegisterViewId, usize)>::new();
            for (partner_domain, pending) in partners {
                let v_mask = viable_masks[partner_domain];
                let candidate_v_mask = v_mask & search_mask;
                if candidate_v_mask == 0 {
                    continue;
                }
                assigned.clear();
                for affinity in affinities.get_domain_affinities(partner_domain) {
                    if let Some(home) = homes.get(affinity.partner.0 as usize).copied().flatten() {
                        if let Some((_, count)) = assigned.iter_mut().find(|(v, _)| *v == home) {
                            *count += 1;
                        } else {
                            assigned.push((home, 1));
                        }
                    }
                }
                let mut peak = 0usize;
                let mut peak_view = RegisterViewId(v_mask.trailing_zeros() as u16);
                for &(av, count) in &assigned {
                    if (v_mask & (1u128 << av.0)) != 0
                        && (count > peak || (count == peak && av < peak_view))
                    {
                        peak = count;
                        peak_view = av;
                    }
                }
                let partner_vote_mask = if pending > peak {
                    candidate_v_mask
                } else if pending == peak {
                    let mut vm = (1u128 << peak_view.0) - 1;
                    for &(av, _) in &assigned {
                        vm |= 1u128 << av.0;
                    }
                    vm & candidate_v_mask
                } else {
                    let mut vm = 0u128;
                    for &(av, count) in &assigned {
                        let drawn = count + pending;
                        if drawn > peak || (drawn == peak && av < peak_view) {
                            vm |= 1u128 << av.0;
                        }
                    }
                    vm & candidate_v_mask
                };
                if single_partner {
                    return if partner_vote_mask != 0 {
                        partner_vote_mask
                    } else {
                        search_mask
                    };
                }
                let mut pvm = partner_vote_mask;
                while pvm != 0 {
                    let bit = pvm.trailing_zeros() as u16;
                    pvm &= pvm - 1;
                    let bit_mask = 1u128 << bit;
                    let idx = usize::from(bit);
                    let total = if (voted_mask & bit_mask) == 0 {
                        voted_mask |= bit_mask;
                        pending
                    } else {
                        usize::from(vote_counts[idx]) + pending
                    };
                    vote_counts[idx] = total as u16;
                    if total > max_votes {
                        max_votes = total;
                        best_votes_mask = bit_mask;
                    } else if total == max_votes {
                        best_votes_mask |= bit_mask;
                    }
                }
            }
            best_votes_mask
        }
    }
    pub(crate) mod prepared_affinities {
        use selected_instructions::{CopyAffinity, VirtualRegisterId};

        #[derive(Clone, Copy)]
        pub(crate) struct DomainAffinity {
            pub(crate) partner: VirtualRegisterId,
            pub(crate) partner_domain: Option<usize>,
        }

        pub(crate) struct PreparedAffinities {
            offsets: Vec<u32>,
            entries: Vec<DomainAffinity>,
        }

        impl PreparedAffinities {
            pub(crate) fn new(
                domain_count: usize,
                domain_of: &[u32],
                affinities: &[CopyAffinity],
            ) -> Self {
                let lookup = |id: VirtualRegisterId| -> Option<usize> {
                    domain_of
                        .get(id.0 as usize)
                        .copied()
                        .filter(|&d| d != u32::MAX)
                        .map(|d| d as usize)
                };
                let mut counts = vec![0u32; domain_count];
                for affinity in affinities {
                    let source_domain = lookup(affinity.source);
                    let destination_domain = lookup(affinity.destination);
                    if let Some(source_domain) = source_domain {
                        counts[source_domain] += 1;
                    }
                    if let Some(destination_domain) = destination_domain
                        && Some(destination_domain) != source_domain
                    {
                        counts[destination_domain] += 1;
                    }
                }
                let mut offsets = vec![0u32; domain_count + 1];
                for i in 0..domain_count {
                    offsets[i + 1] = offsets[i] + counts[i];
                }
                let mut cursors = offsets.clone();
                let mut entries = vec![
                    DomainAffinity {
                        partner: VirtualRegisterId(0),
                        partner_domain: None,
                    };
                    offsets[domain_count] as usize
                ];
                for affinity in affinities {
                    let source_domain = lookup(affinity.source);
                    let destination_domain = lookup(affinity.destination);
                    if let Some(source_domain) = source_domain {
                        let sc = &mut cursors[source_domain];
                        entries[*sc as usize] = DomainAffinity {
                            partner: affinity.destination,
                            partner_domain: destination_domain,
                        };
                        *sc += 1;
                    }
                    if let Some(destination_domain) = destination_domain
                        && Some(destination_domain) != source_domain
                    {
                        let dc = &mut cursors[destination_domain];
                        entries[*dc as usize] = DomainAffinity {
                            partner: affinity.source,
                            partner_domain: source_domain,
                        };
                        *dc += 1;
                    }
                }
                Self { offsets, entries }
            }

            #[inline]
            pub(crate) fn is_empty(&self) -> bool {
                self.entries.is_empty()
            }

            pub(crate) fn get_domain_affinities(&self, domain: usize) -> &[DomainAffinity] {
                let start = self.offsets[domain] as usize;
                let end = self.offsets[domain + 1] as usize;
                &self.entries[start..end]
            }
        }
    }
    pub(crate) mod prepared_conflicts {
        use register_model::{RegisterView, RegisterViewId, ValidatedPhysicalRegisterModel};
        use selected_instructions::{FunctionLiveRanges, VirtualRegisterId};

        use super::domain::AllocationDomain;
        use super::prepared_affinities::PreparedAffinities;
        use crate::RegisterHomeError;

        #[derive(Clone, Copy, Default)]
        pub(crate) struct DomainPair {
            pub(crate) interferes: bool,
            pub(crate) left_defines: bool,
            pub(crate) right_defines: bool,
        }

        impl DomainPair {
            #[inline]
            pub(crate) const fn reversed(self) -> Self {
                Self {
                    interferes: self.interferes,
                    left_defines: self.right_defines,
                    right_defines: self.left_defines,
                }
            }
        }

        pub(crate) struct PreparedConflicts {
            offsets: Vec<u32>,
            entries: Vec<PreparedNeighbor>,
            affinity_offsets: Vec<u32>,
            affinity_entries: Vec<u32>,
            view_count: usize,
            physical_counts: Vec<u16>,
            max_any_physical_conflicts: usize,
            uniform_physical_conflicts: bool,
            physical_matrix: Vec<u8>,
            conflict_masks: Vec<[u128; 8]>,
        }

        const MASK_INTERFERES: u8 = 1;
        const MASK_LEFT_DEFINES: u8 = 2;
        const MASK_RIGHT_DEFINES: u8 = 4;

        impl DomainPair {
            #[inline]
            const fn to_mask(self) -> u8 {
                (if self.interferes { MASK_INTERFERES } else { 0 })
                    | (if self.left_defines {
                        MASK_LEFT_DEFINES
                    } else {
                        0
                    })
                    | (if self.right_defines {
                        MASK_RIGHT_DEFINES
                    } else {
                        0
                    })
            }
        }

        #[derive(Clone, Copy, Default)]
        struct PreparedNeighbor {
            neighbor: [u8; 4],
            relations: u8,
        }

        const _: () = {
            assert!(std::mem::size_of::<PreparedNeighbor>() == 5);
            assert!(std::mem::align_of::<PreparedNeighbor>() == 1);
        };

        impl PreparedNeighbor {
            #[inline]
            fn new(neighbor: u32, pair: DomainPair) -> Self {
                Self {
                    neighbor: neighbor.to_le_bytes(),
                    relations: pair.to_mask(),
                }
            }

            #[inline]
            fn neighbor(self) -> u32 {
                u32::from_le_bytes(self.neighbor)
            }

            #[inline]
            fn pair(self) -> DomainPair {
                DomainPair {
                    interferes: (self.relations & MASK_INTERFERES) != 0,
                    left_defines: (self.relations & MASK_LEFT_DEFINES) != 0,
                    right_defines: (self.relations & MASK_RIGHT_DEFINES) != 0,
                }
            }

            #[inline]
            fn merge(&mut self, other: Self) {
                self.relations |= other.relations;
            }
        }

        impl PreparedConflicts {
            pub(crate) fn new(
                function: usize,
                domains: &[AllocationDomain<'_>],
                domain_of: &[u32],
                ranges: &FunctionLiveRanges,
                physical: &ValidatedPhysicalRegisterModel,
                affinities: &PreparedAffinities,
            ) -> Result<Self, RegisterHomeError> {
                let lookup = |id: VirtualRegisterId| -> Option<usize> {
                    domain_of
                        .get(id.0 as usize)
                        .copied()
                        .filter(|&d| d != u32::MAX)
                        .map(|d| d as usize)
                };
                let domain_count = domains.len();
                let mut counts = vec![0u32; domain_count];
                for interference in &ranges.interference {
                    if let (Some(left), Some(right)) =
                        (lookup(interference.lower), lookup(interference.higher))
                        && left != right
                    {
                        counts[left] += 1;
                        counts[right] += 1;
                    }
                }
                for early_clobber in &ranges.early_clobbers {
                    let Some(definition) = lookup(early_clobber.def_virtual_register) else {
                        continue;
                    };
                    for used in &early_clobber.uses {
                        if let Some(used_domain) = lookup(used.virtual_register)
                            && definition != used_domain
                        {
                            counts[definition] += 1;
                            counts[used_domain] += 1;
                        }
                    }
                }
                let mut offsets = vec![0u32; domain_count + 1];
                for i in 0..domain_count {
                    offsets[i + 1] = offsets[i] + counts[i];
                }
                let mut cursors = offsets.clone();
                let mut entries = vec![PreparedNeighbor::default(); offsets[domain_count] as usize];
                let mut neighbors_strictly_sorted = ranges.early_clobbers.is_empty()
                    && domains.iter().all(|d| d.extra_members.is_empty())
                    && domains.windows(2).all(|w| w[0].leader() < w[1].leader());
                let mut prev_pair = (VirtualRegisterId(0), VirtualRegisterId(0));
                let mut first_pair = true;
                for interference in &ranges.interference {
                    if neighbors_strictly_sorted {
                        if interference.lower >= interference.higher
                            || (!first_pair
                                && (interference.lower, interference.higher) <= prev_pair)
                        {
                            neighbors_strictly_sorted = false;
                        }
                        prev_pair = (interference.lower, interference.higher);
                        first_pair = false;
                    }
                    let (Some(left), Some(right)) =
                        (lookup(interference.lower), lookup(interference.higher))
                    else {
                        continue;
                    };
                    if left == right {
                        continue;
                    }
                    let pair = DomainPair {
                        interferes: true,
                        left_defines: false,
                        right_defines: false,
                    };
                    let lc = &mut cursors[left];
                    entries[*lc as usize] = PreparedNeighbor::new(right as u32, pair);
                    *lc += 1;
                    let rc = &mut cursors[right];
                    entries[*rc as usize] = PreparedNeighbor::new(left as u32, pair);
                    *rc += 1;
                }
                for early_clobber in &ranges.early_clobbers {
                    let Some(definition) = lookup(early_clobber.def_virtual_register) else {
                        continue;
                    };
                    for used in &early_clobber.uses {
                        let Some(used_domain) = lookup(used.virtual_register) else {
                            continue;
                        };
                        if definition == used_domain {
                            continue;
                        }
                        let dc = &mut cursors[definition];
                        entries[*dc as usize] = PreparedNeighbor::new(
                            used_domain as u32,
                            DomainPair {
                                interferes: false,
                                left_defines: true,
                                right_defines: false,
                            },
                        );
                        *dc += 1;
                        let uc = &mut cursors[used_domain];
                        entries[*uc as usize] = PreparedNeighbor::new(
                            definition as u32,
                            DomainPair {
                                interferes: false,
                                left_defines: false,
                                right_defines: true,
                            },
                        );
                        *uc += 1;
                    }
                }
                if !neighbors_strictly_sorted {
                    let mut write_pos = 0usize;
                    let mut domain_offset = vec![u32::MAX; domain_count];
                    for domain in 0..domain_count {
                        let start = offsets[domain] as usize;
                        let end = cursors[domain] as usize;
                        offsets[domain] = write_pos as u32;
                        let len = end - start;
                        if len == 0 {
                            continue;
                        }
                        if len == 1 {
                            if write_pos != start {
                                entries[write_pos] = entries[start];
                            }
                            write_pos += 1;
                            continue;
                        }
                        let row = &mut entries[start..end];
                        if row.windows(2).all(|w| w[0].neighbor() < w[1].neighbor()) {
                            if write_pos != start {
                                entries.copy_within(start..end, write_pos);
                            }
                            write_pos += len;
                            continue;
                        }
                        if len <= 32 {
                            row.sort_unstable_by_key(|entry| entry.neighbor());
                            let mut kept = 0usize;
                            for idx in 1..len {
                                if row[kept].neighbor() == row[idx].neighbor() {
                                    let duplicate = row[idx];
                                    row[kept].merge(duplicate);
                                } else {
                                    kept += 1;
                                    if kept != idx {
                                        row[kept] = row[idx];
                                    }
                                }
                            }
                            let new_len = kept + 1;
                            if write_pos != start {
                                entries.copy_within(start..start + new_len, write_pos);
                            }
                            write_pos += new_len;
                        } else {
                            let mut unique = 0usize;
                            let mut monotonic = true;
                            let mut prev = 0u32;
                            for idx in 0..len {
                                let entry = row[idx];
                                let other = entry.neighbor();
                                let off = domain_offset[other as usize];
                                if off == u32::MAX {
                                    domain_offset[other as usize] = unique as u32;
                                    if unique > 0 && other < prev {
                                        monotonic = false;
                                    }
                                    prev = other;
                                    row[unique] = entry;
                                    unique += 1;
                                } else {
                                    row[off as usize].merge(entry);
                                }
                            }
                            for idx in 0..unique {
                                domain_offset[row[idx].neighbor() as usize] = u32::MAX;
                            }
                            if !monotonic {
                                row[..unique].sort_unstable_by_key(|entry| entry.neighbor());
                            }
                            if write_pos != start {
                                entries.copy_within(start..start + unique, write_pos);
                            }
                            write_pos += unique;
                        }
                    }
                    offsets[domain_count] = write_pos as u32;
                    entries.truncate(write_pos);
                }
                let model = physical.model();
                let view_count = model.views.len();
                let mut used_views = vec![false; view_count];
                let all_domains_under_128 =
                    view_count <= 128 || domains.iter().all(|d| d.candidates.is_empty());
                let dense_view_ids = all_domains_under_128
                    && model
                        .views
                        .iter()
                        .enumerate()
                        .all(|(idx, view)| usize::from(view.id.0) == idx);
                if dense_view_ids {
                    let max_class = model
                        .views
                        .iter()
                        .map(|v| usize::from(v.class.0))
                        .max()
                        .unwrap_or(0);
                    let mut class_masks = vec![0u128; max_class + 1];
                    for (idx, view) in model.views.iter().enumerate().take(128) {
                        class_masks[usize::from(view.class.0)] |= 1u128 << idx;
                    }
                    let mut used_views_mask = 0u128;
                    for domain in domains {
                        let class_idx = usize::from(domain.leader_member.class.0);
                        let valid_mask = class_masks.get(class_idx).copied().unwrap_or(0);
                        if domain.candidates.is_empty() {
                            let invalid = domain.candidates_mask & !valid_mask;
                            if invalid != 0 {
                                return Err(RegisterHomeError::UnknownOrIncompatibleView {
                                    function,
                                    register: domain.leader_member.virtual_register.0,
                                    view: invalid.trailing_zeros() as u16,
                                });
                            }
                            used_views_mask |= domain.candidates_mask;
                        } else {
                            for candidate in &domain.candidates {
                                let bit = usize::from(candidate.0);
                                if bit >= 128 || (valid_mask & (1u128 << bit)) == 0 {
                                    return Err(RegisterHomeError::UnknownOrIncompatibleView {
                                        function,
                                        register: domain.leader_member.virtual_register.0,
                                        view: candidate.0,
                                    });
                                }
                                used_views_mask |= 1u128 << bit;
                            }
                        }
                    }
                    for (idx, slot) in used_views.iter_mut().enumerate().take(128) {
                        *slot = (used_views_mask & (1u128 << idx)) != 0;
                    }
                } else {
                    for domain in domains {
                        if domain.candidates.is_empty() {
                            let mut mask = domain.candidates_mask;
                            while mask != 0 {
                                let bit = mask.trailing_zeros() as u16;
                                mask &= mask - 1;
                                let view = model
                                    .views
                                    .get(usize::from(bit))
                                    .filter(|view| view.class == domain.leader_member.class)
                                    .ok_or(RegisterHomeError::UnknownOrIncompatibleView {
                                        function,
                                        register: domain.leader_member.virtual_register.0,
                                        view: bit,
                                    })?;
                                used_views[usize::from(view.id.0)] = true;
                            }
                        } else {
                            for candidate in &domain.candidates {
                                let view = model
                                    .views
                                    .get(usize::from(candidate.0))
                                    .filter(|view| view.class == domain.leader_member.class)
                                    .ok_or(RegisterHomeError::UnknownOrIncompatibleView {
                                        function,
                                        register: domain.leader_member.virtual_register.0,
                                        view: candidate.0,
                                    })?;
                                used_views[usize::from(view.id.0)] = true;
                            }
                        }
                    }
                }
                let mut physical_counts = vec![0u16; view_count];
                let mut physical_matrix = vec![0u8; view_count * view_count];
                let use_conflict_masks = all_domains_under_128
                    && used_views
                        .iter()
                        .enumerate()
                        .all(|(idx, &used)| !used || idx < 128);
                let mut conflict_masks = if use_conflict_masks {
                    vec![[0u128; 8]; view_count.min(128)]
                } else {
                    Vec::new()
                };
                let can_use_unit_masks = model.views.iter().all(|v| {
                    let idx = usize::from(v.id.0);
                    !used_views.get(idx).copied().unwrap_or(false)
                        || v.units
                            .iter()
                            .chain(&v.write_units)
                            .all(|u| usize::from(u.0) < 128)
                });
                let mut record_pair = |left_idx: usize, right_idx: usize, mask: u8| -> bool {
                    if mask == 0 {
                        return false;
                    }
                    physical_matrix[left_idx * view_count + right_idx] = mask;
                    if use_conflict_masks {
                        let bit = 1u128 << left_idx;
                        let right_masks = &mut conflict_masks[right_idx];
                        for pm in 1..8u8 {
                            if (mask & pm) != 0 {
                                right_masks[pm as usize] |= bit;
                            }
                        }
                    }
                    true
                };
                if can_use_unit_masks {
                    let mut view_masks = vec![(0u128, 0u128, 0u128); view_count];
                    let mut active_indices = Vec::with_capacity(view_count);
                    for view in &model.views {
                        let idx = usize::from(view.id.0);
                        if !used_views[idx] {
                            continue;
                        }
                        let mut u_mask = 0u128;
                        for &u in &view.units {
                            u_mask |= 1u128 << u.0;
                        }
                        let mut w_mask = 0u128;
                        for &u in &view.write_units {
                            w_mask |= 1u128 << u.0;
                        }
                        view_masks[idx] = (u_mask, w_mask, u_mask | w_mask);
                        active_indices.push(idx);
                    }
                    for &left_idx in &active_indices {
                        let (left_u, left_w, left_all) = view_masks[left_idx];
                        let mut count = 0u16;
                        for &right_idx in &active_indices {
                            let (right_u, right_w, right_all) = view_masks[right_idx];
                            let pair = DomainPair {
                                interferes: (left_all & right_all) != 0,
                                left_defines: (left_w & right_u) != 0,
                                right_defines: (right_w & left_u) != 0,
                            };
                            if record_pair(left_idx, right_idx, pair.to_mask()) {
                                count += 1;
                            }
                        }
                        physical_counts[left_idx] = count;
                    }
                } else {
                    for left in &model.views {
                        let left_idx = usize::from(left.id.0);
                        if !used_views[left_idx] {
                            continue;
                        }
                        let mut count = 0u16;
                        for right in &model.views {
                            let right_idx = usize::from(right.id.0);
                            if !used_views[right_idx] {
                                continue;
                            }
                            let pair = DomainPair {
                                interferes: left.footprints_overlap(right),
                                left_defines: definition_overwrites_use(left, right),
                                right_defines: definition_overwrites_use(right, left),
                            };
                            if record_pair(left_idx, right_idx, pair.to_mask()) {
                                count += 1;
                            }
                        }
                        physical_counts[left_idx] = count;
                    }
                }
                let mut max_any_physical_conflicts = 0usize;
                let mut first_used_count = None::<usize>;
                let mut uniform_physical_conflicts = true;
                for (idx, &used) in used_views.iter().enumerate() {
                    if used {
                        let c = physical_counts[idx] as usize;
                        max_any_physical_conflicts = max_any_physical_conflicts.max(c);
                        if let Some(first) = first_used_count {
                            if c != first {
                                uniform_physical_conflicts = false;
                            }
                        } else {
                            first_used_count = Some(c);
                        }
                    }
                }
                let mut affinity_offsets = Vec::new();
                let mut affinity_entries = Vec::new();
                if !affinities.is_empty() {
                    affinity_offsets.reserve(domain_count + 1);
                    affinity_offsets.push(0);
                    for domain in 0..domain_count {
                        let start = offsets[domain] as usize;
                        let end = offsets[domain + 1] as usize;
                        for (position, entry) in entries[start..end].iter().enumerate() {
                            if !affinities
                                .get_domain_affinities(entry.neighbor() as usize)
                                .is_empty()
                            {
                                affinity_entries.push((start + position) as u32);
                            }
                        }
                        affinity_offsets.push(affinity_entries.len() as u32);
                    }
                }
                Ok(Self {
                    offsets,
                    entries,
                    affinity_offsets,
                    affinity_entries,
                    view_count,
                    physical_counts,
                    max_any_physical_conflicts,
                    uniform_physical_conflicts,
                    physical_matrix,
                    conflict_masks,
                })
            }

            #[inline]
            pub(crate) fn uses_bitmasks(&self) -> bool {
                !self.conflict_masks.is_empty()
            }

            #[inline]
            pub(crate) fn max_any_physical_conflicts(&self) -> usize {
                self.max_any_physical_conflicts
            }

            #[inline]
            pub(crate) fn uniform_physical_conflicts(&self) -> bool {
                self.uniform_physical_conflicts
            }

            #[inline]
            pub(crate) fn has_affinity_neighbor(&self, domain: usize) -> bool {
                self.affinity_offsets.get(domain + 1).copied().unwrap_or(0)
                    > self.affinity_offsets.get(domain).copied().unwrap_or(0)
            }

            #[inline]
            pub(crate) fn get_affinity_neighbors_with_pair(
                &self,
                domain: usize,
            ) -> impl Iterator<Item = (usize, DomainPair)> + '_ {
                let start = self.affinity_offsets.get(domain).copied().unwrap_or(0) as usize;
                let end = self.affinity_offsets.get(domain + 1).copied().unwrap_or(0) as usize;
                self.affinity_entries[start..end].iter().map(|&position| {
                    let entry = self.entries[position as usize];
                    (entry.neighbor() as usize, entry.pair())
                })
            }

            #[inline]
            pub(crate) fn degree(&self, domain: usize) -> usize {
                (self.offsets[domain + 1] - self.offsets[domain]) as usize
            }

            #[inline]
            pub(crate) fn conflict_mask(
                &self,
                pair: DomainPair,
                other_view: RegisterViewId,
            ) -> u128 {
                self.conflict_masks[usize::from(other_view.0)][pair.to_mask() as usize]
            }

            #[inline]
            fn row(&self, domain: usize) -> &[PreparedNeighbor] {
                let start = self.offsets[domain] as usize;
                let end = self.offsets[domain + 1] as usize;
                &self.entries[start..end]
            }

            pub(crate) fn get_constrained_neighbors(
                &self,
                domain: usize,
            ) -> impl Iterator<Item = usize> + '_ {
                self.row(domain)
                    .iter()
                    .map(|entry| entry.neighbor() as usize)
            }

            pub(crate) fn get_constrained_neighbors_with_pair(
                &self,
                domain: usize,
            ) -> impl Iterator<Item = (usize, DomainPair)> + '_ {
                self.row(domain)
                    .iter()
                    .map(|entry| (entry.neighbor() as usize, entry.pair()))
            }

            #[inline]
            pub(crate) fn max_physical_conflicts(&self, view: RegisterViewId) -> usize {
                self.physical_counts[usize::from(view.0)] as usize
            }

            #[inline]
            pub(crate) fn conflicts_with_pair(
                &self,
                pair: DomainPair,
                view: RegisterViewId,
                other_view: RegisterViewId,
            ) -> bool {
                let physical_mask = self.physical_matrix
                    [usize::from(view.0) * self.view_count + usize::from(other_view.0)];
                (physical_mask & pair.to_mask()) != 0
            }

            pub(crate) fn constrained(&self, left: usize, right: usize) -> bool {
                let pair = self.pair(left, right);
                pair.interferes || pair.left_defines || pair.right_defines
            }

            fn pair(&self, left: usize, right: usize) -> DomainPair {
                let right_u32 = right as u32;
                let row = self.row(left);
                row.binary_search_by_key(&right_u32, |entry| entry.neighbor())
                    .map_or_else(|_| DomainPair::default(), |position| row[position].pair())
            }
        }

        fn definition_overwrites_use(definition: &RegisterView, used: &RegisterView) -> bool {
            definition
                .write_units
                .iter()
                .any(|unit| used.units.contains(unit))
        }
    }

    use register_environment::ValidatedTargetRegisterEnvironment;
    use register_model::{
        TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
        infrastructure_environment_agrees,
    };

    use std::sync::Arc;

    use crate::{RegisterHomeError, RegisterHomePlan};
    use register_homes::{FunctionAllocationLegality, FunctionRegisterHomes};
    use selected_instruction_analyses::{ValidatedAllocationLegality, ValidatedLiveRanges};
    use selected_instructions::FunctionLiveRanges;

    pub(crate) use placement::compute_function;

    #[derive(Default)]
    pub(crate) struct FunctionHomeReuse {
        register_environment: Option<TargetRegisterEnvironmentIdentity>,
        functions: Vec<Option<ReusableFunctionHomes>>,
        pressure: Vec<RegisterHomeError>,
    }

    struct ReusableFunctionHomes {
        legality: Arc<FunctionAllocationLegality>,
        ranges: Arc<FunctionLiveRanges>,
        homes: FunctionRegisterHomes,
    }

    impl FunctionHomeReuse {
        pub(crate) fn take_pressure(&mut self) -> Vec<RegisterHomeError> {
            std::mem::take(&mut self.pressure)
        }

        pub(crate) fn compute(
            &mut self,
            register_environment: TargetRegisterEnvironmentIdentity,
            index: usize,
            legality: &Arc<FunctionAllocationLegality>,
            ranges: &Arc<FunctionLiveRanges>,
            physical: &ValidatedPhysicalRegisterModel,
        ) -> Result<FunctionRegisterHomes, RegisterHomeError> {
            if self.register_environment != Some(register_environment) {
                self.register_environment = Some(register_environment);
                self.functions.clear();
            }
            if let Some(Some(prior)) = self.functions.get(index)
                && (Arc::ptr_eq(&prior.legality, legality) || prior.legality == *legality)
                && (Arc::ptr_eq(&prior.ranges, ranges) || prior.ranges == *ranges)
            {
                let homes = prior.homes.clone();
                return Ok(homes);
            }
            let homes =
                compute_function(index, legality, ranges, physical, Some(&mut self.pressure))?;
            if self.functions.len() <= index {
                self.functions.resize_with(index + 1, || None);
            }
            self.functions[index] = Some(ReusableFunctionHomes {
                legality: Arc::clone(legality),
                ranges: Arc::clone(ranges),
                homes: homes.clone(),
            });
            Ok(homes)
        }

        pub(crate) fn probe_all(
            &mut self,
            register_environment: TargetRegisterEnvironmentIdentity,
            legality_functions: &[Arc<FunctionAllocationLegality>],
            range_functions: &[Arc<FunctionLiveRanges>],
            physical: &ValidatedPhysicalRegisterModel,
        ) -> Result<(), RegisterHomeError> {
            if legality_functions.len() != range_functions.len() {
                return Err(RegisterHomeError::RootMismatch);
            }
            self.pressure.clear();
            if self.register_environment != Some(register_environment) {
                self.register_environment = Some(register_environment);
                self.functions.clear();
            }
            for (index, (legality, ranges)) in
                legality_functions.iter().zip(range_functions).enumerate()
            {
                if let Some(Some(prior)) = self.functions.get(index)
                    && Arc::ptr_eq(&prior.legality, legality)
                    && Arc::ptr_eq(&prior.ranges, ranges)
                {
                    continue;
                }
                if legality.machine != ranges.machine {
                    return Err(RegisterHomeError::FunctionMismatch { function: index });
                }
                if let Some(Some(prior)) = self.functions.get(index)
                    && prior.legality == *legality
                    && prior.ranges == *ranges
                {
                    continue;
                }
                self.compute(register_environment, index, legality, ranges, physical)?;
            }
            Ok(())
        }
    }

    pub(crate) fn compute_terminal_register_homes(
        legality: &ValidatedAllocationLegality,
        ranges: &ValidatedLiveRanges,
        environment: &ValidatedTargetRegisterEnvironment,
        mut reuse: Option<&mut FunctionHomeReuse>,
    ) -> Result<RegisterHomePlan, RegisterHomeError> {
        let register_environment = environment.identity();
        if legality.receipt().ranges() != ranges.receipt().identity()
            || legality.receipt().register_environment() != register_environment
            || ranges.plan().target.architecture != environment.physical().model().architecture
            || !infrastructure_environment_agrees(
                ranges.plan().target,
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &environment.allocation_constraint_keys(),
                register_environment,
            )
            || legality.plan().functions.len() != ranges.plan().functions.len()
        {
            return Err(RegisterHomeError::RootMismatch);
        }
        if let Some(reuse) = reuse.as_deref_mut() {
            reuse.pressure.clear();
        }
        let functions = legality
            .plan()
            .functions
            .iter()
            .zip(&ranges.plan().functions)
            .enumerate()
            .map(|(index, (legality, ranges))| {
                if legality.machine != ranges.machine {
                    return Err(RegisterHomeError::FunctionMismatch { function: index });
                }
                match reuse.as_deref_mut() {
                    Some(reuse) => reuse.compute(
                        register_environment,
                        index,
                        legality,
                        ranges,
                        environment.physical(),
                    ),
                    None => compute_function(index, legality, ranges, environment.physical(), None),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RegisterHomePlan {
            legality: legality.receipt().identity(),
            ranges: ranges.receipt().identity(),
            register_environment,
            allocator_availability: legality.receipt().allocator_availability(),
            functions,
        })
    }
}
pub(crate) mod physical_requirement {
    use crate::RegisterHomeError;
    use register_homes::VirtualRegisterAllocationLegality;
    use selected_instructions::{FunctionLiveRanges, VirtualLiveRange, VirtualRegisterId};

    pub(crate) struct ReferencedRegisters(Vec<u64>);

    impl ReferencedRegisters {
        #[inline]
        fn insert(&mut self, reg: VirtualRegisterId) {
            let word = (reg.0 as usize) >> 6;
            if word >= self.0.len() {
                self.0.resize(word + 1, 0);
            }
            self.0[word] |= 1u64 << (reg.0 & 63);
        }

        #[inline]
        pub(crate) fn contains(&self, reg: VirtualRegisterId) -> bool {
            let word = (reg.0 as usize) >> 6;
            self.0
                .get(word)
                .is_some_and(|&bits| (bits & (1u64 << (reg.0 & 63))) != 0)
        }
    }

    pub(crate) fn referenced_in_relations(ranges: &FunctionLiveRanges) -> ReferencedRegisters {
        let max_reg = ranges
            .virtual_registers
            .iter()
            .map(|r| r.virtual_register.0 as usize)
            .max()
            .unwrap_or(0);
        let mut referenced = ReferencedRegisters(vec![0u64; (max_reg >> 6) + 1]);
        for tie in &ranges.tied_pairs {
            referenced.insert(tie.use_virtual_register);
            referenced.insert(tie.def_virtual_register);
        }
        for edge in &ranges.edge_transfers {
            referenced.insert(edge.argument);
            referenced.insert(edge.parameter);
        }
        for affinity in &ranges.copy_affinities {
            referenced.insert(affinity.source);
            referenced.insert(affinity.destination);
        }
        for early in &ranges.early_clobbers {
            referenced.insert(early.def_virtual_register);
            for used in &early.uses {
                referenced.insert(used.virtual_register);
            }
        }
        for pair in &ranges.interference {
            referenced.insert(pair.lower);
            referenced.insert(pair.higher);
        }
        referenced
    }

    pub(crate) fn validate(
        function: usize,
        legality: &VirtualRegisterAllocationLegality,
        range: &VirtualLiveRange,
        ranges: &FunctionLiveRanges,
        referenced_cache: &mut Option<ReferencedRegisters>,
    ) -> Result<(), RegisterHomeError> {
        let register = legality.virtual_register;
        if register != range.virtual_register || legality.class != range.class {
            return Err(RegisterHomeError::VirtualRegisterMismatch {
                function,
                register: register.0,
            });
        }
        if !legality.spans.is_empty() {
            return Ok(());
        }
        if !legality.entry_transitions.is_empty() {
            return Err(RegisterHomeError::UnresolvedEntryTransitions {
                function,
                register: register.0,
                count: legality.entry_transitions.len(),
            });
        }
        if !legality.early_clobber_points.is_empty()
            || !range.occurrences.is_empty()
            || !range.fragments.is_empty()
            || !range.edge_connectors.is_empty()
            || !range.fixed_constraints.is_empty()
            || referenced_cache
                .get_or_insert_with(|| referenced_in_relations(ranges))
                .contains(register)
        {
            return Err(RegisterHomeError::NoLivePoints {
                function,
                register: register.0,
            });
        }
        Ok(())
    }
}
pub(crate) mod receipt {
    use std::collections::BTreeSet;

    use selected_instructions::VirtualRegisterId;

    use crate::{RegisterHomePlan, RegisterHomeValidationReceipt, register_home_identity};
    use selected_instruction_analyses::ValidatedLiveRanges;
    use selected_instructions::DistinctUseDefTie;

    pub(crate) fn build(
        plan: &RegisterHomePlan,
        ranges: &ValidatedLiveRanges,
    ) -> RegisterHomeValidationReceipt {
        RegisterHomeValidationReceipt {
            identity: register_home_identity(plan),
            legality: plan.legality,
            ranges: plan.ranges,
            register_environment: plan.register_environment,
            allocator_availability: plan.allocator_availability,
            function_count: plan.functions.len(),
            assignment_count: plan
                .functions
                .iter()
                .map(|function| function.assignments.len())
                .sum(),
            tied_pair_count: ranges
                .plan()
                .functions
                .iter()
                .map(|function| function.tied_pairs.len())
                .sum(),
            tied_component_count: ranges
                .plan()
                .functions
                .iter()
                .map(|function| tied_component_count(&function.tied_pairs))
                .sum(),
            early_clobber_count: ranges
                .plan()
                .functions
                .iter()
                .map(|function| function.early_clobbers.len())
                .sum(),
        }
    }

    fn tied_component_count(ties: &[DistinctUseDefTie]) -> usize {
        let mut components = Vec::<BTreeSet<VirtualRegisterId>>::new();
        for tie in ties {
            let used = components
                .iter()
                .position(|component| component.contains(&tie.use_virtual_register));
            let defined = components
                .iter()
                .position(|component| component.contains(&tie.def_virtual_register));
            match (used, defined) {
                (None, None) => components.push(BTreeSet::from([
                    tie.use_virtual_register,
                    tie.def_virtual_register,
                ])),
                (Some(component), None) => {
                    components[component].insert(tie.def_virtual_register);
                }
                (None, Some(component)) => {
                    components[component].insert(tie.use_virtual_register);
                }
                (Some(left), Some(right)) if left != right => {
                    let (keep, remove) = if left < right {
                        (left, right)
                    } else {
                        (right, left)
                    };
                    let removed = components.remove(remove);
                    components[keep].extend(removed);
                }
                (Some(_), Some(_)) => {}
            }
        }
        components.len()
    }
}

use register_homes::{AllocationLegalityIdentity, AllocatorAvailabilityIdentity};
pub use register_homes::{
    FunctionRegisterHomes, RegisterHomeDecodeError, RegisterHomeIdentity, RegisterHomePlan,
    VirtualRegisterHome, register_home_identity,
};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::LiveRangeIdentity;

pub(crate) fn assign_register_homes(
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedRegisterHomes, RegisterHomeError> {
    assign_register_homes_reusing(legality, ranges, environment, None)
}

pub(crate) fn assign_register_homes_reusing(
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    environment: &ValidatedTargetRegisterEnvironment,
    reuse: Option<&mut compute::FunctionHomeReuse>,
) -> Result<ValidatedRegisterHomes, RegisterHomeError> {
    let plan = compute::compute_terminal_register_homes(legality, ranges, environment, reuse)?;
    let receipt = receipt::build(&plan, ranges);
    Ok(ValidatedRegisterHomes {
        plan: plan.into(),
        receipt,
    })
}

copy_receipt! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RegisterHomeValidationReceipt {
        pub(crate) identity: RegisterHomeIdentity,
        pub(crate) legality: AllocationLegalityIdentity,
        pub(crate) ranges: LiveRangeIdentity,
        pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
        pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
        pub(crate) function_count: usize,
        pub(crate) assignment_count: usize,
        pub(crate) tied_pair_count: usize,
        pub(crate) tied_component_count: usize,
        pub(crate) early_clobber_count: usize,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRegisterHomes {
    pub(crate) plan: std::sync::Arc<RegisterHomePlan>,
    pub(crate) receipt: RegisterHomeValidationReceipt,
}

impl ValidatedRegisterHomes {
    pub fn shared_plan(&self) -> std::sync::Arc<RegisterHomePlan> {
        std::sync::Arc::clone(&self.plan)
    }

    pub fn plan(&self) -> &RegisterHomePlan {
        &self.plan
    }
    receipt_accessor_rows!(ref ValidatedRegisterHomes {
        receipt: RegisterHomeValidationReceipt,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterHomeError {
    RootMismatch,
    FunctionMismatch {
        function: usize,
    },
    VirtualRegisterMismatch {
        function: usize,
        register: u32,
    },
    UnresolvedEntryTransitions {
        function: usize,
        register: u32,
        count: usize,
    },
    NoLivePoints {
        function: usize,
        register: u32,
    },
    IntervalOverflow {
        function: usize,
        register: u32,
    },
    NoCommonCandidate {
        function: usize,
        register: u32,
    },
    UnknownOrIncompatibleView {
        function: usize,
        register: u32,
        view: u16,
    },
    NoCompatibleHome {
        function: usize,
        register: u32,
    },
    UnsupportedTiedTopology {
        function: usize,
        instruction: u32,
    },
    UnsupportedEdgeTransfer {
        function: usize,
        edge: u64,
    },
    TiedRegistersInterfere {
        function: usize,
        lower: u32,
        higher: u32,
    },
    NoCommonTiedComponent {
        function: usize,
        leader: u32,
        member_count: usize,
    },
}

impl RegisterHomeError {
    pub(crate) const fn admits_runtime_recovery(&self) -> bool {
        matches!(
            self,
            Self::NoCompatibleHome { .. }
                | Self::NoCommonCandidate { .. }
                | Self::UnresolvedEntryTransitions { .. }
        )
    }
}

stage_error_display!(
    RegisterHomeError,
    "Terminal register-home assignment failed"
);

//! Tied-component construction and common physical-view domains.

use register_model::RegisterViewId;
use selected_instructions::VirtualRegisterId;

use super::conflicts::registers_interfere;
use crate::RegisterHomeError;
use register_homes::{FunctionAllocationLegality, VirtualRegisterAllocationLegality};
use selected_instructions::{FunctionLiveRanges, LiveRangePoint};

#[derive(Debug)]
pub(super) struct AllocationDomain<'a> {
    pub(super) leader_member: &'a VirtualRegisterAllocationLegality,
    pub(super) extra_members: Vec<&'a VirtualRegisterAllocationLegality>,
    pub(super) first_point: LiveRangePoint,
    pub(super) candidates_mask: u128,
    pub(super) candidates: Vec<RegisterViewId>,
}

impl<'a> AllocationDomain<'a> {
    #[inline]
    pub(super) fn leader(&self) -> VirtualRegisterId {
        self.leader_member.virtual_register
    }

    #[inline]
    pub(super) fn members(
        &self,
    ) -> std::iter::Chain<
        std::iter::Once<&'a VirtualRegisterAllocationLegality>,
        std::iter::Copied<std::slice::Iter<'_, &'a VirtualRegisterAllocationLegality>>,
    > {
        std::iter::once(self.leader_member).chain(self.extra_members.iter().copied())
    }

    pub(super) fn materialize_candidates(&self) -> Vec<RegisterViewId> {
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

pub(super) fn build_domains<'a>(
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
        .map(|( mut leader, mut extra)| match build_domain(function, &mut leader, &mut extra) {
            Ok((first_point, cands)) => Ok(AllocationDomain {
                leader_member: leader,
                extra_members: extra,
                first_point,
                candidates_mask: cands.mask,
                candidates: cands.overflow,
            }),
            Err(error) => {
                // A failed component reports the edge transfer naming one of its
                // members. The search runs only on that error path: searching
                // every transfer for every component made domain building
                // quadratic in a large function.
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
        let (Some(argument), Some(parameter)) = (
            get_pos(transfer.argument),
            get_pos(transfer.parameter),
        ) else {
            return Err(error());
        };
        if legality.virtual_registers[argument].class != transfer.class
            || legality.virtual_registers[parameter].class != transfer.class
        {
            return Err(error());
        }
    }
    let components = crate::assignment::same_home::SameHomeComponents::from_ranges(ranges);
    let mut grouped: Vec<Option<(
        &'a VirtualRegisterAllocationLegality,
        Vec<&'a VirtualRegisterAllocationLegality>,
    )>> = vec![None; legality.virtual_registers.len()];
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
                        && early.uses.iter().any(|used| has_member(used.virtual_register))
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
            for right in extra {
                if registers_interfere(
                    leader.virtual_register,
                    right.virtual_register,
                    &ranges.interference,
                ) {
                    let (lower, higher) =
                        ordered_pair(leader.virtual_register, right.virtual_register);
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
                    return Err(RegisterHomeError::TiedRegistersInterfere {
                        function,
                        lower: lower.0,
                        higher: higher.0,
                    });
                }
            }
            for (index, left) in extra.iter().enumerate() {
                for right in extra.iter().skip(index + 1) {
                    if registers_interfere(
                        left.virtual_register,
                        right.virtual_register,
                        &ranges.interference,
                    ) {
                        let (lower, higher) =
                            ordered_pair(left.virtual_register, right.virtual_register);
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
                        return Err(RegisterHomeError::TiedRegistersInterfere {
                            function,
                            lower: lower.0,
                            higher: higher.0,
                        });
                    }
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
        first_point = Some(first_point.map_or(first, |point: LiveRangePoint| point.min(first)));
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
    Ok((
        first_point.expect("allocation domain is nonempty"),
        cands,
    ))
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

fn interval_bounds(
    function: usize,
    register: &VirtualRegisterAllocationLegality,
) -> Result<(LiveRangePoint, LiveRangePoint), RegisterHomeError> {
    let first = register
        .spans
        .first()
        .ok_or(RegisterHomeError::NoLivePoints {
            function,
            register: register.virtual_register.0,
        })?;
    let last = register.spans.last().expect("nonempty spans established");
    Ok((first.start, last.end))
}

fn common_candidates_repr(
    function: usize,
    register: &VirtualRegisterAllocationLegality,
) -> Result<DomainCandidates, RegisterHomeError> {
    let first = register
        .spans
        .first()
        .ok_or(RegisterHomeError::NoLivePoints {
            function,
            register: register.virtual_register.0,
        })?;
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
        // Each phase can have a legal home while one unsplit lifetime cannot:
        // for example, a value survives a call and later has a fixed use.
        // Invite the existing validated recovery to split or spill that range;
        // no home is admitted until its rebuilt legality and assignment pass.
        // Inspect every point independently: intersection stops at its first
        // empty result, and an impossible individual phase must still refuse.
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
    if common
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
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

    /// The vector intersection equals the set intersection it replaced.
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
            let actual = intersect_candidates(&lists[0], lists[1..].iter().map(Vec::as_slice))
                .into_iter()
                .collect::<BTreeSet<_>>();
            assert_eq!(actual, expected);
        }
    }
}

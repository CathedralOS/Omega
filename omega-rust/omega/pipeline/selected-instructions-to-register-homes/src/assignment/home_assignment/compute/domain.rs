//! Tied-component construction and common physical-view domains.

use std::collections::{BTreeMap, BTreeSet};

use register_model::RegisterViewId;
use selected_instructions::VirtualRegisterId;

use super::conflicts::registers_interfere;
use crate::{LiveRangePoint, RegisterHomeError};

#[derive(Debug)]
pub(super) struct AllocationDomain<'a> {
    pub(super) members: Vec<&'a crate::VirtualRegisterAllocationLegality>,
    pub(super) first_point: LiveRangePoint,
    pub(super) candidates: BTreeSet<RegisterViewId>,
}

impl AllocationDomain<'_> {
    pub(super) fn leader(&self) -> VirtualRegisterId {
        self.members[0].virtual_register
    }

    pub(super) fn contains(&self, register: VirtualRegisterId) -> bool {
        self.members
            .binary_search_by_key(&register, |member| member.virtual_register)
            .is_ok()
    }
}

pub(super) fn build_domains<'a>(
    function: usize,
    legality: &'a crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
) -> Result<Vec<AllocationDomain<'a>>, RegisterHomeError> {
    tied_components(function, legality, ranges)?
        .into_iter()
        .map(|members| build_domain(function, members))
        .collect()
}

fn tied_components<'a>(
    function: usize,
    legality: &'a crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
) -> Result<Vec<Vec<&'a crate::VirtualRegisterAllocationLegality>>, RegisterHomeError> {
    let positions = legality
        .virtual_registers
        .iter()
        .enumerate()
        .map(|(position, register)| (register.virtual_register, position))
        .collect::<BTreeMap<_, _>>();
    let mut parents = (0..legality.virtual_registers.len()).collect::<Vec<_>>();
    for tie in &ranges.tied_pairs {
        let (Some(&used), Some(&defined)) = (
            positions.get(&tie.use_virtual_register),
            positions.get(&tie.def_virtual_register),
        ) else {
            return Err(unsupported_tie(function, tie.instruction.0));
        };
        if used == defined
            || legality.virtual_registers[used].class != tie.class
            || legality.virtual_registers[defined].class != tie.class
        {
            return Err(unsupported_tie(function, tie.instruction.0));
        }
        let used_root = component_root(&parents, used);
        let defined_root = component_root(&parents, defined);
        if used_root != defined_root {
            parents[defined_root] = used_root;
        }
    }
    let mut grouped = BTreeMap::<usize, Vec<_>>::new();
    for (position, register) in legality.virtual_registers.iter().enumerate() {
        grouped
            .entry(component_root(&parents, position))
            .or_default()
            .push(register);
    }
    for members in grouped.values() {
        for (index, left) in members.iter().enumerate() {
            for right in members.iter().skip(index + 1) {
                if registers_interfere(
                    left.virtual_register,
                    right.virtual_register,
                    &ranges.interference,
                ) {
                    let (lower, higher) =
                        ordered_pair(left.virtual_register, right.virtual_register);
                    return Err(RegisterHomeError::TiedRegistersInterfere {
                        function,
                        lower: lower.0,
                        higher: higher.0,
                    });
                }
            }
        }
    }
    Ok(grouped.into_values().collect())
}

fn build_domain<'a>(
    function: usize,
    mut members: Vec<&'a crate::VirtualRegisterAllocationLegality>,
) -> Result<AllocationDomain<'a>, RegisterHomeError> {
    members.sort_by_key(|member| member.virtual_register);
    let mut first_point = None;
    let mut candidates = None::<BTreeSet<RegisterViewId>>;
    for member in &members {
        if !member.entry_transitions.is_empty() {
            return Err(RegisterHomeError::UnresolvedEntryTransitions {
                function,
                register: member.virtual_register.0,
                count: member.entry_transitions.len(),
            });
        }
        let (first, _) = interval_bounds(function, member)?;
        first_point = Some(first_point.map_or(first, |point: LiveRangePoint| point.min(first)));
        let member_candidates = common_candidates(function, member)?;
        if let Some(shared) = &mut candidates {
            shared.retain(|candidate| member_candidates.contains(candidate));
        } else {
            candidates = Some(member_candidates);
        }
    }
    let candidates = candidates.expect("allocation domain is nonempty");
    if candidates.is_empty() {
        if members.len() > 1 {
            return Err(RegisterHomeError::NoCommonTiedComponent {
                function,
                leader: members[0].virtual_register.0,
                member_count: members.len(),
            });
        }
        return Err(RegisterHomeError::NoCommonCandidate {
            function,
            register: members[0].virtual_register.0,
        });
    }
    Ok(AllocationDomain {
        members,
        first_point: first_point.expect("allocation domain is nonempty"),
        candidates,
    })
}

fn interval_bounds(
    function: usize,
    register: &crate::VirtualRegisterAllocationLegality,
) -> Result<(LiveRangePoint, LiveRangePoint), RegisterHomeError> {
    let first = register
        .points
        .first()
        .ok_or(RegisterHomeError::NoLivePoints {
            function,
            register: register.virtual_register.0,
        })?;
    let last = register.points.last().expect("nonempty points established");
    let end = last.point.0.checked_add(1).map(LiveRangePoint).ok_or(
        RegisterHomeError::IntervalOverflow {
            function,
            register: register.virtual_register.0,
        },
    )?;
    Ok((first.point, end))
}

fn common_candidates(
    function: usize,
    register: &crate::VirtualRegisterAllocationLegality,
) -> Result<BTreeSet<RegisterViewId>, RegisterHomeError> {
    let first = register
        .points
        .first()
        .ok_or(RegisterHomeError::NoLivePoints {
            function,
            register: register.virtual_register.0,
        })?;
    let mut common = first.candidates.iter().copied().collect::<BTreeSet<_>>();
    for point in &register.points[1..] {
        common.retain(|candidate| point.candidates.binary_search(candidate).is_ok());
    }
    for point in &register.early_clobber_points {
        common.retain(|candidate| point.candidates.binary_search(candidate).is_ok());
    }
    if common.is_empty() {
        return Err(RegisterHomeError::NoCommonCandidate {
            function,
            register: register.virtual_register.0,
        });
    }
    Ok(common)
}

fn component_root(parents: &[usize], mut position: usize) -> usize {
    while parents[position] != position {
        position = parents[position];
    }
    position
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

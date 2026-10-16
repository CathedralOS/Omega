//! Tied-component construction and common physical-view domains.

use std::collections::{BTreeMap, BTreeSet};

use register_model::RegisterViewId;
use selected_instructions::VirtualRegisterId;

use super::conflicts::registers_interfere;
use crate::RegisterHomeError;
use register_homes::{FunctionAllocationLegality, VirtualRegisterAllocationLegality};
use selected_instructions::{FunctionLiveRanges, LiveRangePoint};

#[derive(Debug)]
pub(super) struct AllocationDomain<'a> {
    pub(super) members: Vec<&'a VirtualRegisterAllocationLegality>,
    pub(super) first_point: LiveRangePoint,
    pub(super) candidates: BTreeSet<RegisterViewId>,
}

impl AllocationDomain<'_> {
    pub(super) fn leader(&self) -> VirtualRegisterId {
        self.members[0].virtual_register
    }
}

pub(super) fn build_domains<'a>(
    function: usize,
    legality: &'a FunctionAllocationLegality,
    ranges: &FunctionLiveRanges,
) -> Result<Vec<AllocationDomain<'a>>, RegisterHomeError> {
    for (register, range) in legality
        .virtual_registers
        .iter()
        .zip(&ranges.virtual_registers)
    {
        super::super::physical_requirement::validate(function, register, range, ranges)?;
    }
    tied_components(function, legality, ranges)?
        .into_iter()
        .filter(|members| members.iter().any(|member| !member.spans.is_empty()))
        .map(|members| {
            // A failed component reports the edge transfer naming one of its
            // members. The search runs only on that error path: searching
            // every transfer for every component made domain building
            // quadratic in a large function.
            let component = members.clone();
            build_domain(function, members).map_err(|error| {
                let edge = || {
                    ranges.edge_transfers.iter().find(|edge| {
                        component
                            .iter()
                            .any(|member| member.virtual_register == edge.parameter)
                    })
                };
                if matches!(error, RegisterHomeError::NoCommonTiedComponent { .. })
                    && let Some(edge) = edge()
                {
                    RegisterHomeError::UnsupportedEdgeTransfer {
                        function,
                        edge: edge.psi_edge.get(),
                    }
                } else {
                    error
                }
            })
        })
        .collect()
}

fn tied_components<'a>(
    function: usize,
    legality: &'a FunctionAllocationLegality,
    ranges: &FunctionLiveRanges,
) -> Result<Vec<Vec<&'a VirtualRegisterAllocationLegality>>, RegisterHomeError> {
    let positions = legality
        .virtual_registers
        .iter()
        .enumerate()
        .map(|(position, register)| (register.virtual_register, position))
        .collect::<BTreeMap<_, _>>();
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
    }
    for transfer in &ranges.edge_transfers {
        let error = || RegisterHomeError::UnsupportedEdgeTransfer {
            function,
            edge: transfer.psi_edge.get(),
        };
        let (Some(&argument), Some(&parameter)) = (
            positions.get(&transfer.argument),
            positions.get(&transfer.parameter),
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
    let mut grouped = BTreeMap::<usize, Vec<_>>::new();
    for register in &legality.virtual_registers {
        grouped
            .entry(positions[&components.leader(register.virtual_register)])
            .or_default()
            .push(register);
    }
    for members in grouped.values() {
        if ranges.early_clobbers.iter().any(|early| {
            members
                .iter()
                .any(|member| member.virtual_register == early.def_virtual_register)
                && early.uses.iter().any(|used| {
                    members
                        .iter()
                        .any(|member| member.virtual_register == used.virtual_register)
                })
        }) && let Some(edge) = ranges.edge_transfers.iter().find(|edge| {
            members
                .iter()
                .any(|member| member.virtual_register == edge.parameter)
        }) {
            return Err(RegisterHomeError::UnsupportedEdgeTransfer {
                function,
                edge: edge.psi_edge.get(),
            });
        }
        for (index, left) in members.iter().enumerate() {
            for right in members.iter().skip(index + 1) {
                if registers_interfere(
                    left.virtual_register,
                    right.virtual_register,
                    &ranges.interference,
                ) {
                    let (lower, higher) =
                        ordered_pair(left.virtual_register, right.virtual_register);
                    if let Some(edge) = ranges.edge_transfers.iter().find(|edge| {
                        members
                            .iter()
                            .any(|member| member.virtual_register == edge.parameter)
                    }) {
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
    Ok(grouped.into_values().collect())
}

fn build_domain<'a>(
    function: usize,
    mut members: Vec<&'a VirtualRegisterAllocationLegality>,
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

fn common_candidates(
    function: usize,
    register: &VirtualRegisterAllocationLegality,
) -> Result<BTreeSet<RegisterViewId>, RegisterHomeError> {
    let first = register
        .spans
        .first()
        .ok_or(RegisterHomeError::NoLivePoints {
            function,
            register: register.virtual_register.0,
        })?;
    let common = intersect_candidates(
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

/// The views legal at every point of a register's range. Each point's
/// candidates are canonically view-sorted, so the running intersection stays a
/// sorted vector filtered by binary search; a long-lived register crosses
/// thousands of points, and spill recovery recomputes every register of the
/// changed function at each step.
fn intersect_candidates<'a>(
    first: &[RegisterViewId],
    rest: impl Iterator<Item = &'a [RegisterViewId]>,
) -> BTreeSet<RegisterViewId> {
    let mut common = first
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    for candidates in rest {
        if common.is_empty() {
            break;
        }
        common.retain(|candidate| candidates.binary_search(candidate).is_ok());
    }
    common.into_iter().collect()
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
            let actual = intersect_candidates(&lists[0], lists[1..].iter().map(Vec::as_slice));
            assert_eq!(actual, expected);
        }
    }
}

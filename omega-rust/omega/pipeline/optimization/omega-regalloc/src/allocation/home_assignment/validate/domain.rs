//! Independent tied-component and candidate-domain reconstruction.

use std::collections::{BTreeMap, BTreeSet};

use omega_register_model::{RegisterClassId, RegisterViewId};
use omega_selected_instructions::VirtualRegisterId;

use super::conflicts;
use crate::{LiveRangePoint, RegisterHomeError};

#[derive(Debug, Clone)]
pub(super) struct ReplayDomain {
    pub(super) registers: Vec<VirtualRegisterId>,
    pub(super) leader: VirtualRegisterId,
    pub(super) class: RegisterClassId,
    pub(super) earliest_point: LiveRangePoint,
    pub(super) candidates: Vec<RegisterViewId>,
}

pub(super) fn reconstruct(
    function: usize,
    legality: &crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
) -> Result<Vec<ReplayDomain>, RegisterHomeError> {
    if legality.virtual_registers.len() != ranges.virtual_registers.len() {
        return Err(RegisterHomeError::FunctionMismatch { function });
    }
    let positions = legality
        .virtual_registers
        .iter()
        .enumerate()
        .map(|(position, register)| (register.virtual_register, position))
        .collect::<BTreeMap<_, _>>();
    let mut components = legality
        .virtual_registers
        .iter()
        .map(|register| BTreeSet::from([register.virtual_register]))
        .collect::<Vec<_>>();
    for tie in &ranges.tied_pairs {
        let (Some(&used), Some(&defined)) = (
            positions.get(&tie.use_virtual_register),
            positions.get(&tie.def_virtual_register),
        ) else {
            return Err(RegisterHomeError::UnsupportedTiedTopology {
                function,
                instruction: tie.instruction.0,
            });
        };
        if used == defined
            || legality.virtual_registers[used].class != tie.class
            || legality.virtual_registers[defined].class != tie.class
        {
            return Err(RegisterHomeError::UnsupportedTiedTopology {
                function,
                instruction: tie.instruction.0,
            });
        }
        let used_component = component_containing(&components, tie.use_virtual_register);
        let defined_component = component_containing(&components, tie.def_virtual_register);
        if used_component != defined_component {
            let (keep, remove) = if used_component < defined_component {
                (used_component, defined_component)
            } else {
                (defined_component, used_component)
            };
            let removed = components.remove(remove);
            components[keep].extend(removed);
        }
    }
    components.sort_by_key(|component| component.first().copied());
    components
        .into_iter()
        .map(|component| build(function, component, legality, ranges))
        .collect()
}

fn component_containing(
    components: &[BTreeSet<VirtualRegisterId>],
    register: VirtualRegisterId,
) -> usize {
    components
        .iter()
        .position(|component| component.contains(&register))
        .expect("known register retains exactly one replay component")
}

fn build(
    function: usize,
    component: BTreeSet<VirtualRegisterId>,
    legality: &crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
) -> Result<ReplayDomain, RegisterHomeError> {
    let members = legality
        .virtual_registers
        .iter()
        .enumerate()
        .filter_map(|(position, register)| {
            component
                .contains(&register.virtual_register)
                .then_some(position)
        })
        .collect::<Vec<_>>();
    let registers = component.into_iter().collect::<Vec<_>>();
    let leader = registers[0];
    reject_internal_interference(function, &registers, ranges)?;
    let mut earliest = None;
    let mut shared = None::<BTreeSet<RegisterViewId>>;
    for member in &members {
        let register = &legality.virtual_registers[*member];
        if !register.entry_transitions.is_empty() {
            return Err(RegisterHomeError::UnresolvedEntryTransitions {
                function,
                register: register.virtual_register.0,
                count: register.entry_transitions.len(),
            });
        }
        let (start, _) = interval(function, register)?;
        earliest = Some(earliest.map_or(start, |point: LiveRangePoint| point.min(start)));
        let candidates = candidates(function, register)?;
        if let Some(common) = &mut shared {
            common.retain(|candidate| candidates.contains(candidate));
        } else {
            shared = Some(candidates);
        }
    }
    let shared = shared.expect("replay component is nonempty");
    if shared.is_empty() {
        if members.len() > 1 {
            return Err(RegisterHomeError::NoCommonTiedComponent {
                function,
                leader: leader.0,
                member_count: members.len(),
            });
        }
        return Err(RegisterHomeError::NoCommonCandidate {
            function,
            register: leader.0,
        });
    }
    let class = legality.virtual_registers[*members.first().expect("nonempty component")].class;
    Ok(ReplayDomain {
        registers,
        leader,
        class,
        earliest_point: earliest.expect("nonempty component"),
        candidates: shared.into_iter().collect(),
    })
}

fn reject_internal_interference(
    function: usize,
    registers: &[VirtualRegisterId],
    ranges: &crate::FunctionLiveRanges,
) -> Result<(), RegisterHomeError> {
    for (left_index, left) in registers.iter().enumerate() {
        for right in registers.iter().skip(left_index + 1) {
            if conflicts::interferes(*left, *right, &ranges.interference) {
                return Err(RegisterHomeError::TiedRegistersInterfere {
                    function,
                    lower: left.0.min(right.0),
                    higher: left.0.max(right.0),
                });
            }
        }
    }
    Ok(())
}

fn interval(
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

fn candidates(
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

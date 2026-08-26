use std::collections::BTreeSet;

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{
    RegisterView, RegisterViewId, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};
use omega_terminal_selected_instructions::TerminalVirtualRegisterId;

use crate::{
    TerminalFunctionSpillChoices, TerminalLiveRangePoint, TerminalPressureContender,
    TerminalPressureResident, TerminalSpillChoice, TerminalSpillChoiceError,
    TerminalSpillChoicePlan, TerminalSpillChoicePolicy, TerminalVirtualInterference,
    ValidatedTerminalAllocationLegality, ValidatedTerminalLiveRanges,
};

#[derive(Debug, Clone, Copy)]
struct ActiveHome {
    register: TerminalVirtualRegisterId,
    class: omega_register_model::RegisterClassId,
    start: TerminalLiveRangePoint,
    end: TerminalLiveRangePoint,
    view: RegisterViewId,
}

#[derive(Default)]
struct WorkCounter {
    rule_evaluations: u64,
    candidates: u64,
    validation_steps: u64,
    commits: u64,
}

impl WorkCounter {
    fn add(value: &mut u64, amount: u64) -> Result<(), TerminalSpillChoiceError> {
        *value = value
            .checked_add(amount)
            .ok_or(TerminalSpillChoiceError::WorkOverflow)?;
        Ok(())
    }
    fn usage(&self) -> OptimizationWorkUsage {
        OptimizationWorkUsage {
            rule_evaluations: self.rule_evaluations,
            candidates: self.candidates,
            validation_steps: self.validation_steps,
            commits: self.commits,
            iterations: 1,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_terminal_spill_choices(
    legality: &ValidatedTerminalAllocationLegality,
    ranges: &ValidatedTerminalLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: TerminalSpillChoicePolicy,
    budget: OptimizationWorkBudget,
) -> Result<TerminalSpillChoicePlan, TerminalSpillChoiceError> {
    validate_roots(
        legality,
        ranges,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
    )?;
    if policy != TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1 {
        return Err(TerminalSpillChoiceError::UnsupportedPolicy);
    }
    let mut work = WorkCounter::default();
    let mut functions = Vec::with_capacity(legality.plan().functions.len());
    for (function_index, (legality_function, range_function)) in legality
        .plan()
        .functions
        .iter()
        .zip(&ranges.plan().functions)
        .enumerate()
    {
        functions.push(compute_function(
            function_index,
            legality_function,
            range_function,
            physical,
            &mut work,
        )?);
    }
    let usage = work.usage();
    if !usage.within(budget) {
        return Err(TerminalSpillChoiceError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    Ok(TerminalSpillChoicePlan {
        legality: legality.receipt().identity(),
        ranges: ranges.receipt().identity(),
        register_environment,
        policy,
        budget,
        usage,
        functions,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_roots(
    legality: &ValidatedTerminalAllocationLegality,
    ranges: &ValidatedTerminalLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<(), TerminalSpillChoiceError> {
    if legality.receipt().ranges() != ranges.receipt().identity()
        || legality.receipt().register_environment() != register_environment
        || ranges.plan().target.architecture != physical.model().architecture
        || constraints.physical_identity() != physical.identity()
        || reservations.physical_identity() != physical.identity()
        || reservations.target() != ranges.plan().target
        || target_register_environment_identity(
            ranges.plan().target,
            physical,
            constraints,
            reservations,
            selected_keys,
        ) != register_environment
        || legality.plan().functions.len() != ranges.plan().functions.len()
    {
        return Err(TerminalSpillChoiceError::RootMismatch);
    }
    Ok(())
}

fn compute_function(
    function_index: usize,
    legality: &crate::TerminalFunctionAllocationLegality,
    ranges: &crate::TerminalFunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    work: &mut WorkCounter,
) -> Result<TerminalFunctionSpillChoices, TerminalSpillChoiceError> {
    if legality.machine != ranges.machine
        || legality.virtual_registers.len() != ranges.virtual_registers.len()
    {
        return Err(TerminalSpillChoiceError::FunctionMismatch {
            function: function_index,
        });
    }
    let mut order = legality
        .virtual_registers
        .iter()
        .enumerate()
        .map(|(position, register)| {
            interval_bounds(function_index, register).map(|(start, end)| (position, start, end))
        })
        .collect::<Result<Vec<_>, _>>()?;
    order.sort_by_key(|(position, start, _)| {
        (
            start.0,
            legality.virtual_registers[*position].virtual_register,
        )
    });
    let mut active = Vec::<ActiveHome>::new();
    for (position, start, end) in order {
        WorkCounter::add(&mut work.rule_evaluations, 1)?;
        let register = &legality.virtual_registers[position];
        let range = ranges
            .virtual_registers
            .get(position)
            .filter(|range| {
                range.virtual_register == register.virtual_register && range.class == register.class
            })
            .ok_or(TerminalSpillChoiceError::VirtualRegisterMismatch {
                function: function_index,
                register: register.virtual_register.0,
            })?;
        if !register.entry_transitions.is_empty() {
            return Err(TerminalSpillChoiceError::UnresolvedEntryTransitions {
                function: function_index,
                register: register.virtual_register.0,
            });
        }
        active.retain(|entry| entry.end > start);
        let candidates = common_candidates(function_index, register)?;
        let mut selected = None;
        for candidate in &candidates {
            WorkCounter::add(&mut work.validation_steps, 1)?;
            let view = checked_view(
                function_index,
                register.virtual_register,
                register.class,
                *candidate,
                physical,
            )?;
            if !blocked(
                register.virtual_register,
                view,
                &active,
                &ranges.interference,
                physical,
                None,
            ) {
                selected = Some(*candidate);
                break;
            }
        }
        if let Some(view) = selected {
            active.push(ActiveHome {
                register: register.virtual_register,
                class: register.class,
                start,
                end,
                view,
            });
            active.sort_by_key(|entry| (entry.end, entry.register));
            continue;
        }

        let block = register.points[0].block;
        if register.points.iter().any(|point| point.block != block) {
            return Err(TerminalSpillChoiceError::UnsupportedPressureShape {
                function: function_index,
                register: register.virtual_register.0,
            });
        }
        validate_local_shape(
            function_index,
            register.virtual_register,
            start,
            end,
            block,
            range,
        )?;
        for resident in &active {
            let resident_range = ranges
                .virtual_registers
                .iter()
                .find(|candidate| candidate.virtual_register == resident.register)
                .ok_or(TerminalSpillChoiceError::VirtualRegisterMismatch {
                    function: function_index,
                    register: resident.register.0,
                })?;
            validate_local_shape(
                function_index,
                resident.register,
                resident.start,
                resident.end,
                block,
                resident_range,
            )?;
        }
        if active.iter().any(|resident| {
            legality
                .virtual_registers
                .iter()
                .find(|candidate| candidate.virtual_register == resident.register)
                .is_none_or(|candidate| candidate.points.iter().any(|point| point.block != block))
        }) {
            return Err(TerminalSpillChoiceError::UnsupportedPressureShape {
                function: function_index,
                register: register.virtual_register.0,
            });
        }
        let active_residents = active
            .iter()
            .map(|entry| TerminalPressureResident {
                virtual_register: entry.register,
                class: entry.class,
                start: entry.start,
                exclusive_end: entry.end,
                view: entry.view,
            })
            .collect::<Vec<_>>();
        let mut contenders = vec![TerminalPressureContender {
            virtual_register: register.virtual_register,
            exclusive_end: end,
            reclaimed_view: None,
        }];
        for resident in &active {
            let mut reclaimed = None;
            for candidate in &candidates {
                WorkCounter::add(&mut work.validation_steps, 1)?;
                let view = checked_view(
                    function_index,
                    register.virtual_register,
                    register.class,
                    *candidate,
                    physical,
                )?;
                if !blocked(
                    register.virtual_register,
                    view,
                    &active,
                    &ranges.interference,
                    physical,
                    Some(resident.register),
                ) {
                    reclaimed = Some(*candidate);
                    break;
                }
            }
            if let Some(view) = reclaimed {
                contenders.push(TerminalPressureContender {
                    virtual_register: resident.register,
                    exclusive_end: resident.end,
                    reclaimed_view: Some(view),
                });
            }
        }
        contenders.sort_by_key(|contender| contender.virtual_register);
        WorkCounter::add(
            &mut work.candidates,
            u64::try_from(contenders.len()).map_err(|_| TerminalSpillChoiceError::WorkOverflow)?,
        )?;
        WorkCounter::add(&mut work.commits, 1)?;
        let selected_victim = contenders
            .iter()
            .max_by_key(|contender| (contender.exclusive_end, contender.virtual_register))
            .expect("incoming contender is always present")
            .virtual_register;
        return Ok(TerminalFunctionSpillChoices {
            machine: legality.machine,
            choice: Some(TerminalSpillChoice {
                block,
                point: start,
                incoming: register.virtual_register,
                incoming_class: register.class,
                incoming_common_candidates: candidates.into_iter().collect(),
                active_residents,
                contenders,
                selected_victim,
            }),
        });
    }
    Ok(TerminalFunctionSpillChoices {
        machine: legality.machine,
        choice: None,
    })
}

fn validate_local_shape(
    function: usize,
    register: TerminalVirtualRegisterId,
    start: TerminalLiveRangePoint,
    end: TerminalLiveRangePoint,
    block: omega_terminal_selected_instructions::TerminalSelectedBlockId,
    range: &crate::TerminalVirtualLiveRange,
) -> Result<(), TerminalSpillChoiceError> {
    if !range.edge_connectors.is_empty() || range.fragments.len() != 1 {
        return Err(TerminalSpillChoiceError::UnsupportedPressureShape {
            function,
            register: register.0,
        });
    }
    let fragment = range.fragments[0];
    if fragment.block != block || fragment.start != start || fragment.end != end {
        return Err(TerminalSpillChoiceError::UnsupportedPressureShape {
            function,
            register: register.0,
        });
    }
    Ok(())
}

fn interval_bounds(
    function: usize,
    register: &crate::TerminalVirtualRegisterAllocationLegality,
) -> Result<(TerminalLiveRangePoint, TerminalLiveRangePoint), TerminalSpillChoiceError> {
    let first = register
        .points
        .first()
        .ok_or(TerminalSpillChoiceError::NoLivePoints {
            function,
            register: register.virtual_register.0,
        })?;
    let last = register.points.last().expect("nonempty points established");
    let end = last
        .point
        .0
        .checked_add(1)
        .map(TerminalLiveRangePoint)
        .ok_or(TerminalSpillChoiceError::IntervalOverflow {
            function,
            register: register.virtual_register.0,
        })?;
    Ok((first.point, end))
}

fn common_candidates(
    function: usize,
    register: &crate::TerminalVirtualRegisterAllocationLegality,
) -> Result<BTreeSet<RegisterViewId>, TerminalSpillChoiceError> {
    let first = register
        .points
        .first()
        .ok_or(TerminalSpillChoiceError::NoLivePoints {
            function,
            register: register.virtual_register.0,
        })?;
    let mut common = first.candidates.iter().copied().collect::<BTreeSet<_>>();
    for point in &register.points[1..] {
        common.retain(|candidate| point.candidates.binary_search(candidate).is_ok());
    }
    if common.is_empty() {
        return Err(TerminalSpillChoiceError::NoCommonCandidate {
            function,
            register: register.virtual_register.0,
        });
    }
    Ok(common)
}

fn checked_view(
    function: usize,
    register: TerminalVirtualRegisterId,
    class: omega_register_model::RegisterClassId,
    candidate: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<&RegisterView, TerminalSpillChoiceError> {
    physical
        .model()
        .views
        .get(usize::from(candidate.0))
        .filter(|view| view.id == candidate && view.class == class)
        .ok_or(TerminalSpillChoiceError::UnknownOrIncompatibleView {
            function,
            register: register.0,
            view: candidate.0,
        })
}

fn blocked(
    incoming: TerminalVirtualRegisterId,
    view: &RegisterView,
    active: &[ActiveHome],
    interference: &[TerminalVirtualInterference],
    physical: &ValidatedPhysicalRegisterModel,
    omitted: Option<TerminalVirtualRegisterId>,
) -> bool {
    active
        .iter()
        .filter(|entry| Some(entry.register) != omitted)
        .any(|entry| {
            interferes(incoming, entry.register, interference)
                && footprints_overlap(view, &physical.model().views[usize::from(entry.view.0)])
        })
}

fn interferes(
    left: TerminalVirtualRegisterId,
    right: TerminalVirtualRegisterId,
    interference: &[TerminalVirtualInterference],
) -> bool {
    let pair = if left < right {
        TerminalVirtualInterference {
            lower: left,
            higher: right,
        }
    } else {
        TerminalVirtualInterference {
            lower: right,
            higher: left,
        }
    };
    interference.binary_search(&pair).is_ok()
}

fn footprints_overlap(left: &RegisterView, right: &RegisterView) -> bool {
    left.units
        .iter()
        .chain(&left.write_units)
        .any(|unit| right.units.contains(unit) || right.write_units.contains(unit))
}

#[cfg(test)]
mod tests {
    use omega_register_model::{
        PhysicalRegisterModel, RegisterClass, RegisterClassId, RegisterUnit, RegisterUnitId,
        RegisterUnitKind, RegisterView, RegisterViewId, RegisterWriteSemantics,
        validate_physical_register_model,
    };
    use omega_terminal_selected_instructions::{
        TerminalSelectedBlockId, TerminalVirtualRegisterId,
    };
    use psi_core::MachineId;

    use super::*;
    use crate::{
        TerminalFunctionAllocationLegality, TerminalFunctionLiveRanges, TerminalLiveRangeFragment,
        TerminalVirtualLiveRange, TerminalVirtualPointLegality,
        TerminalVirtualRegisterAllocationLegality,
    };

    fn physical() -> ValidatedPhysicalRegisterModel {
        validate_physical_register_model(PhysicalRegisterModel {
            architecture: omega_target::Architecture::X86_64,
            units: (0..2)
                .map(|index| RegisterUnit {
                    id: RegisterUnitId(index),
                    name: format!("r{index}.storage"),
                    bits: 64,
                    kind: RegisterUnitKind::IntegerLane,
                })
                .collect(),
            views: (0..2)
                .map(|index| RegisterView {
                    id: RegisterViewId(index),
                    name: format!("r{index}"),
                    class: RegisterClassId(0),
                    units: vec![RegisterUnitId(index)],
                    write_units: vec![RegisterUnitId(index)],
                    bits: 64,
                    write_semantics: RegisterWriteSemantics::ExactView,
                    allocatable: true,
                })
                .collect(),
            classes: vec![RegisterClass {
                id: RegisterClassId(0),
                name: "integer".into(),
                views: vec![RegisterViewId(0), RegisterViewId(1)],
            }],
            conventions: Vec::new(),
            reservations: Vec::new(),
        })
        .unwrap()
    }

    fn legality(intervals: &[(u32, u32)]) -> TerminalFunctionAllocationLegality {
        TerminalFunctionAllocationLegality {
            machine: MachineId::new(1).unwrap(),
            virtual_registers: intervals
                .iter()
                .enumerate()
                .map(
                    |(index, (start, inclusive_end))| TerminalVirtualRegisterAllocationLegality {
                        virtual_register: TerminalVirtualRegisterId(index as u32),
                        class: RegisterClassId(0),
                        points: (*start..=*inclusive_end)
                            .map(|point| TerminalVirtualPointLegality {
                                block: TerminalSelectedBlockId(0),
                                point: TerminalLiveRangePoint(point),
                                candidates: vec![RegisterViewId(0), RegisterViewId(1)],
                            })
                            .collect(),
                        entry_transitions: Vec::new(),
                    },
                )
                .collect(),
        }
    }

    fn ranges(intervals: &[(u32, u32)]) -> TerminalFunctionLiveRanges {
        TerminalFunctionLiveRanges {
            machine: MachineId::new(1).unwrap(),
            block_domains: Vec::new(),
            virtual_registers: intervals
                .iter()
                .enumerate()
                .map(|(index, (start, inclusive_end))| TerminalVirtualLiveRange {
                    virtual_register: TerminalVirtualRegisterId(index as u32),
                    class: RegisterClassId(0),
                    occurrences: Vec::new(),
                    fixed_constraints: Vec::new(),
                    fragments: vec![TerminalLiveRangeFragment {
                        block: TerminalSelectedBlockId(0),
                        start: TerminalLiveRangePoint(*start),
                        end: TerminalLiveRangePoint(inclusive_end + 1),
                    }],
                    edge_connectors: Vec::new(),
                })
                .collect(),
            architectural_units: Vec::new(),
            interference: vec![(0, 1), (0, 2), (1, 2)]
                .into_iter()
                .map(|(lower, higher)| TerminalVirtualInterference {
                    lower: TerminalVirtualRegisterId(lower),
                    higher: TerminalVirtualRegisterId(higher),
                })
                .collect(),
        }
    }

    fn computed(intervals: &[(u32, u32)]) -> (TerminalFunctionSpillChoices, OptimizationWorkUsage) {
        let legality = legality(intervals);
        let ranges = ranges(intervals);
        let physical = physical();
        let mut work = WorkCounter::default();
        let result = compute_function(0, &legality, &ranges, &physical, &mut work).unwrap();
        let replay = crate::spill_choice_validate::replay_function_for_test(
            0, &legality, &ranges, &physical,
        )
        .unwrap();
        assert_eq!((result.clone(), work.usage()), replay);
        (result, work.usage())
    }

    #[test]
    fn equal_end_pressure_keeps_existing_homes_and_selects_the_incoming_value() {
        let (function, usage) = computed(&[(0, 3), (0, 3), (0, 3)]);
        let choice = function.choice.unwrap();
        assert_eq!(choice.selected_victim, TerminalVirtualRegisterId(2));
        assert_eq!(choice.active_residents.len(), 2);
        assert_eq!(choice.contenders.len(), 3);
        assert_eq!(choice.contenders[0].reclaimed_view, Some(RegisterViewId(0)));
        assert_eq!(choice.contenders[1].reclaimed_view, Some(RegisterViewId(1)));
        assert_eq!(choice.contenders[2].reclaimed_view, None);
        assert_eq!(usage.commits, 1);
    }

    #[test]
    fn farther_active_end_wins_only_when_its_eviction_recovers_a_view() {
        let (function, _) = computed(&[(0, 5), (0, 3), (0, 3)]);
        assert_eq!(
            function.choice.unwrap().selected_victim,
            TerminalVirtualRegisterId(0)
        );
    }

    #[test]
    fn unsupported_cross_block_pressure_cannot_issue_local_victim_authority() {
        let mut legality = legality(&[(0, 3), (0, 3), (0, 3)]);
        legality.virtual_registers[2].points[3].block = TerminalSelectedBlockId(1);
        let ranges = ranges(&[(0, 3), (0, 3), (0, 3)]);
        let mut work = WorkCounter::default();
        assert_eq!(
            compute_function(0, &legality, &ranges, &physical(), &mut work),
            Err(TerminalSpillChoiceError::UnsupportedPressureShape {
                function: 0,
                register: 2
            })
        );
    }
}

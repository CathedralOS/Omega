use std::collections::BTreeSet;

use optimization_core::OptimizationWorkUsage;
use register_model::{
    RegisterView, RegisterViewId, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};
use selected_instructions::VirtualRegisterId;

use crate::{
    FunctionSpillChoices, LiveRangePoint, PressureContender, PressureResident, SpillChoice,
    SpillChoiceError, SpillChoicePlan, SpillChoicePolicy, SpillChoiceValidationReceipt,
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedSpillChoices, VirtualInterference,
    spill_choice_identity,
};

#[derive(Clone, Copy)]
struct ReplayResident {
    register: VirtualRegisterId,
    class: register_model::RegisterClassId,
    start: LiveRangePoint,
    end: LiveRangePoint,
    view: RegisterViewId,
}

#[derive(Default)]
struct ReplayWork {
    rules: u64,
    candidates: u64,
    steps: u64,
    commits: u64,
}

impl ReplayWork {
    fn bump(field: &mut u64, amount: u64) -> Result<(), SpillChoiceError> {
        *field = field
            .checked_add(amount)
            .ok_or(SpillChoiceError::WorkOverflow)?;
        Ok(())
    }
    fn finish(&self) -> OptimizationWorkUsage {
        OptimizationWorkUsage {
            rule_evaluations: self.rules,
            candidates: self.candidates,
            validation_steps: self.steps,
            commits: self.commits,
            iterations: 1,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn validate_spill_choices(
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    plan: SpillChoicePlan,
) -> Result<ValidatedSpillChoices, SpillChoiceError> {
    if plan.legality != legality.receipt().identity()
        || plan.ranges != ranges.receipt().identity()
        || plan.register_environment != register_environment
        || plan.allocator_availability != legality.receipt().allocator_availability()
        || legality.receipt().ranges() != ranges.receipt().identity()
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
        || plan.functions.len() != legality.plan().functions.len()
        || plan.functions.len() != ranges.plan().functions.len()
    {
        return Err(SpillChoiceError::RootMismatch);
    }
    if plan.policy != SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1 {
        return Err(SpillChoiceError::UnsupportedPolicy);
    }
    let mut work = ReplayWork::default();
    for (function_index, plan_function) in plan.functions.iter().enumerate() {
        if !ranges.plan().functions[function_index]
            .tied_pairs
            .is_empty()
        {
            return Err(SpillChoiceError::UnsupportedTiedOperands {
                function: function_index,
            });
        }
        if !ranges.plan().functions[function_index]
            .early_clobbers
            .is_empty()
        {
            return Err(SpillChoiceError::UnsupportedEarlyClobber {
                function: function_index,
            });
        }
        let expected = replay_function(
            function_index,
            &legality.plan().functions[function_index],
            &ranges.plan().functions[function_index],
            physical,
            &mut work,
        )?;
        if plan_function != &expected {
            return Err(SpillChoiceError::ChoiceMismatch {
                function: function_index,
            });
        }
    }
    let expected_usage = work.finish();
    if plan.usage != expected_usage {
        return Err(SpillChoiceError::UsageMismatch);
    }
    if !plan.usage.within(plan.budget) {
        return Err(SpillChoiceError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let choice_count = plan
        .functions
        .iter()
        .filter(|function| function.choice.is_some())
        .count();
    let contender_count = plan
        .functions
        .iter()
        .filter_map(|function| function.choice.as_ref())
        .map(|choice| choice.contenders.len())
        .sum();
    let receipt = SpillChoiceValidationReceipt {
        identity: spill_choice_identity(&plan),
        legality: plan.legality,
        ranges: plan.ranges,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        policy: plan.policy,
        usage: plan.usage,
        function_count: plan.functions.len(),
        choice_count,
        contender_count,
    };
    Ok(ValidatedSpillChoices { plan, receipt })
}

fn replay_function(
    function: usize,
    legality: &crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    work: &mut ReplayWork,
) -> Result<FunctionSpillChoices, SpillChoiceError> {
    if legality.machine != ranges.machine
        || legality.virtual_registers.len() != ranges.virtual_registers.len()
    {
        return Err(SpillChoiceError::FunctionMismatch { function });
    }
    let mut schedule = Vec::with_capacity(legality.virtual_registers.len());
    for (position, register) in legality.virtual_registers.iter().enumerate() {
        let first = register
            .points
            .first()
            .ok_or(SpillChoiceError::NoLivePoints {
                function,
                register: register.virtual_register.0,
            })?;
        let last = register.points.last().expect("nonempty points established");
        let end = LiveRangePoint(last.point.0.checked_add(1).ok_or(
            SpillChoiceError::IntervalOverflow {
                function,
                register: register.virtual_register.0,
            },
        )?);
        schedule.push((position, first.point, end));
    }
    schedule.sort_by(|left, right| {
        left.1.cmp(&right.1).then_with(|| {
            legality.virtual_registers[left.0]
                .virtual_register
                .cmp(&legality.virtual_registers[right.0].virtual_register)
        })
    });
    let mut residents = Vec::<ReplayResident>::new();
    for (position, start, end) in schedule {
        ReplayWork::bump(&mut work.rules, 1)?;
        let register = &legality.virtual_registers[position];
        let range = ranges
            .virtual_registers
            .get(position)
            .filter(|range| {
                range.virtual_register == register.virtual_register && range.class == register.class
            })
            .ok_or(SpillChoiceError::VirtualRegisterMismatch {
                function,
                register: register.virtual_register.0,
            })?;
        if !register.entry_transitions.is_empty() {
            return Err(SpillChoiceError::UnresolvedEntryTransitions {
                function,
                register: register.virtual_register.0,
            });
        }
        residents.retain(|resident| resident.end > start);
        let common = replay_common(function, register)?;
        let mut home = None;
        for candidate in &common {
            ReplayWork::bump(&mut work.steps, 1)?;
            let view = replay_view(
                function,
                register.virtual_register,
                register.class,
                *candidate,
                physical,
            )?;
            if residents.iter().all(|resident| {
                !replay_interferes(
                    register.virtual_register,
                    resident.register,
                    &ranges.interference,
                ) || !replay_overlap(view, replay_view_by_id(resident.view, physical))
            }) {
                home = Some(*candidate);
                break;
            }
        }
        if let Some(view) = home {
            residents.push(ReplayResident {
                register: register.virtual_register,
                class: register.class,
                start,
                end,
                view,
            });
            residents.sort_by(|left, right| {
                left.end
                    .cmp(&right.end)
                    .then(left.register.cmp(&right.register))
            });
            continue;
        }
        if register
            .points
            .iter()
            .any(|point| point.block != register.points[0].block)
        {
            return Err(SpillChoiceError::UnsupportedPressureShape {
                function,
                register: register.virtual_register.0,
            });
        }
        replay_shape(
            function,
            register.virtual_register,
            start,
            end,
            register.points[0].block,
            range,
        )?;
        for resident in &residents {
            let resident_legality = legality
                .virtual_registers
                .iter()
                .find(|candidate| candidate.virtual_register == resident.register)
                .ok_or(SpillChoiceError::VirtualRegisterMismatch {
                    function,
                    register: resident.register.0,
                })?;
            let resident_range = ranges
                .virtual_registers
                .iter()
                .find(|candidate| candidate.virtual_register == resident.register)
                .ok_or(SpillChoiceError::VirtualRegisterMismatch {
                    function,
                    register: resident.register.0,
                })?;
            replay_shape(
                function,
                resident.register,
                resident.start,
                resident.end,
                register.points[0].block,
                resident_range,
            )?;
            if resident_legality
                .points
                .iter()
                .any(|point| point.block != register.points[0].block)
            {
                return Err(SpillChoiceError::UnsupportedPressureShape {
                    function,
                    register: resident.register.0,
                });
            }
        }
        let mut contenders = vec![PressureContender {
            virtual_register: register.virtual_register,
            exclusive_end: end,
            reclaimed_view: None,
        }];
        for omitted in &residents {
            let mut reclaimed_view = None;
            for candidate in &common {
                ReplayWork::bump(&mut work.steps, 1)?;
                let incoming_view = replay_view(
                    function,
                    register.virtual_register,
                    register.class,
                    *candidate,
                    physical,
                )?;
                let remains_blocked = residents
                    .iter()
                    .filter(|resident| resident.register != omitted.register)
                    .any(|resident| {
                        replay_interferes(
                            register.virtual_register,
                            resident.register,
                            &ranges.interference,
                        ) && replay_overlap(
                            incoming_view,
                            replay_view_by_id(resident.view, physical),
                        )
                    });
                if !remains_blocked {
                    reclaimed_view = Some(*candidate);
                    break;
                }
            }
            if let Some(view) = reclaimed_view {
                contenders.push(PressureContender {
                    virtual_register: omitted.register,
                    exclusive_end: omitted.end,
                    reclaimed_view: Some(view),
                });
            }
        }
        contenders.sort_by_key(|contender| contender.virtual_register);
        ReplayWork::bump(
            &mut work.candidates,
            u64::try_from(contenders.len()).map_err(|_| SpillChoiceError::WorkOverflow)?,
        )?;
        ReplayWork::bump(&mut work.commits, 1)?;
        let selected_victim = contenders
            .iter()
            .max_by(|left, right| {
                left.exclusive_end
                    .cmp(&right.exclusive_end)
                    .then(left.virtual_register.cmp(&right.virtual_register))
            })
            .expect("incoming contender exists")
            .virtual_register;
        return Ok(FunctionSpillChoices {
            machine: legality.machine,
            choice: Some(SpillChoice {
                block: register.points[0].block,
                point: start,
                incoming: register.virtual_register,
                incoming_class: register.class,
                incoming_common_candidates: common.into_iter().collect(),
                active_residents: residents
                    .iter()
                    .map(|resident| PressureResident {
                        virtual_register: resident.register,
                        class: resident.class,
                        start: resident.start,
                        exclusive_end: resident.end,
                        view: resident.view,
                    })
                    .collect(),
                contenders,
                selected_victim,
            }),
        });
    }
    Ok(FunctionSpillChoices {
        machine: legality.machine,
        choice: None,
    })
}

#[cfg(test)]
pub(crate) fn replay_function_for_test(
    function: usize,
    legality: &crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(FunctionSpillChoices, OptimizationWorkUsage), SpillChoiceError> {
    let mut work = ReplayWork::default();
    let choices = replay_function(function, legality, ranges, physical, &mut work)?;
    Ok((choices, work.finish()))
}

fn replay_common(
    function: usize,
    register: &crate::VirtualRegisterAllocationLegality,
) -> Result<BTreeSet<RegisterViewId>, SpillChoiceError> {
    let first = register
        .points
        .first()
        .ok_or(SpillChoiceError::NoLivePoints {
            function,
            register: register.virtual_register.0,
        })?;
    let mut common = first.candidates.iter().copied().collect::<BTreeSet<_>>();
    for candidate in &first.candidates {
        if register
            .points
            .iter()
            .skip(1)
            .any(|point| !point.candidates.contains(candidate))
        {
            common.remove(candidate);
        }
    }
    if common.is_empty() {
        return Err(SpillChoiceError::NoCommonCandidate {
            function,
            register: register.virtual_register.0,
        });
    }
    Ok(common)
}

fn replay_shape(
    function: usize,
    register: VirtualRegisterId,
    start: LiveRangePoint,
    end: LiveRangePoint,
    block: selected_instructions::SelectedBlockId,
    range: &crate::VirtualLiveRange,
) -> Result<(), SpillChoiceError> {
    if !range.edge_connectors.is_empty() || range.fragments.len() != 1 {
        return Err(SpillChoiceError::UnsupportedPressureShape {
            function,
            register: register.0,
        });
    }
    let fragment = range.fragments[0];
    if fragment.block != block || fragment.start != start || fragment.end != end {
        return Err(SpillChoiceError::UnsupportedPressureShape {
            function,
            register: register.0,
        });
    }
    Ok(())
}

fn replay_view(
    function: usize,
    register: VirtualRegisterId,
    class: register_model::RegisterClassId,
    id: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<&RegisterView, SpillChoiceError> {
    physical
        .model()
        .views
        .iter()
        .find(|view| view.id == id && view.class == class)
        .ok_or(SpillChoiceError::UnknownOrIncompatibleView {
            function,
            register: register.0,
            view: id.0,
        })
}

fn replay_view_by_id(
    id: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> &RegisterView {
    physical
        .model()
        .views
        .iter()
        .find(|view| view.id == id)
        .expect("validated assigned view remains present")
}

fn replay_interferes(
    left: VirtualRegisterId,
    right: VirtualRegisterId,
    pairs: &[VirtualInterference],
) -> bool {
    pairs.iter().any(|pair| {
        (pair.lower == left && pair.higher == right) || (pair.lower == right && pair.higher == left)
    })
}

fn replay_overlap(left: &RegisterView, right: &RegisterView) -> bool {
    let right_units = right
        .units
        .iter()
        .chain(&right.write_units)
        .copied()
        .collect::<BTreeSet<_>>();
    left.units
        .iter()
        .chain(&left.write_units)
        .any(|unit| right_units.contains(unit))
}

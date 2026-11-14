/*
  Deterministic bounded register-pressure spill-victim selection
  (`choose_spill_victims`, `ValidatedSpillChoices`).

  Evaluates allocation legality deficits and interference degree under a
  `SpillChoicePolicy` to select deterministic spill candidates within the
  configured optimization work budget.
*/

use crate::{ValidatedAllocationLegality, ValidatedLiveRanges};
use register_homes::SpillChoicePolicy;
use register_model::{
    PreparedRegisterConstraintCatalog, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterReservationProfile,
};

pub mod compute {
    use std::collections::BTreeSet;

    use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
    use register_model::{
        PreparedRegisterConstraintCatalog, RegisterView, RegisterViewId,
        TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
        ValidatedPhysicalRegisterModel, ValidatedRegisterReservationProfile,
        infrastructure_environment_agrees,
    };
    use selected_instructions::VirtualRegisterId;

    use crate::work_accounting::{Work, count};
    use crate::{SpillChoiceError, ValidatedAllocationLegality, ValidatedLiveRanges};
    use register_homes::{
        FunctionSpillChoices, PressureContender, PressureResident, SpillChoice, SpillChoicePlan,
        SpillChoicePolicy,
    };
    use selected_instructions::{LiveRangePoint, VirtualInterference, VirtualInterferences};

    #[derive(Debug, Clone, Copy)]
    struct ActiveHome {
        register: VirtualRegisterId,
        class: register_model::RegisterClassId,
        start: LiveRangePoint,
        end: LiveRangePoint,
        view: RegisterViewId,
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn compute_terminal_spill_choices(
        legality: &ValidatedAllocationLegality,
        ranges: &ValidatedLiveRanges,
        register_environment: TargetRegisterEnvironmentIdentity,
        physical: &ValidatedPhysicalRegisterModel,
        constraints: &PreparedRegisterConstraintCatalog,
        reservations: &ValidatedRegisterReservationProfile,
        selected_keys: &TargetRegisterEnvironmentConstraintKeys,
        policy: SpillChoicePolicy,
        budget: OptimizationWorkBudget,
    ) -> Result<SpillChoicePlan, SpillChoiceError> {
        validate_roots(
            legality,
            ranges,
            register_environment,
            physical,
            constraints,
            reservations,
            selected_keys,
        )?;
        if policy != SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1 {
            return Err(SpillChoiceError::UnsupportedPolicy);
        }
        let mut work = Work::new(OptimizationWorkUsage {
            iterations: 1,
            ..Default::default()
        });
        let mut functions = Vec::with_capacity(legality.plan().functions.len());
        for (function_index, (legality_function, range_function)) in legality
            .plan()
            .functions
            .iter()
            .zip(&ranges.plan().functions)
            .enumerate()
        {
            reject_constraint_topologies(function_index, range_function)?;
            functions.push(compute_function(
                function_index,
                legality_function,
                range_function,
                physical,
                &mut work,
            )?);
        }
        let usage = work.finish(budget)?;
        Ok(SpillChoicePlan {
            legality: legality.receipt().identity(),
            ranges: ranges.receipt().identity(),
            register_environment,
            allocator_availability: legality.receipt().allocator_availability(),
            policy,
            budget,
            usage,
            functions,
        })
    }

    fn reject_constraint_topologies(
        function: usize,
        ranges: &selected_instructions::FunctionLiveRanges,
    ) -> Result<(), SpillChoiceError> {
        if !ranges.tied_pairs.is_empty() {
            return Err(SpillChoiceError::UnsupportedTiedOperands { function });
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_roots(
        legality: &ValidatedAllocationLegality,
        ranges: &ValidatedLiveRanges,
        register_environment: TargetRegisterEnvironmentIdentity,
        physical: &ValidatedPhysicalRegisterModel,
        constraints: &PreparedRegisterConstraintCatalog,
        reservations: &ValidatedRegisterReservationProfile,
        selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    ) -> Result<(), SpillChoiceError> {
        if legality.receipt().ranges() != ranges.receipt().identity()
            || legality.receipt().register_environment() != register_environment
            || ranges.plan().target.architecture != physical.model().architecture
            || !infrastructure_environment_agrees(
                ranges.plan().target,
                physical,
                constraints,
                reservations,
                selected_keys,
                register_environment,
            )
            || legality.plan().functions.len() != ranges.plan().functions.len()
        {
            return Err(SpillChoiceError::RootMismatch);
        }
        Ok(())
    }

    fn compute_function(
        function_index: usize,
        legality: &register_homes::FunctionAllocationLegality,
        ranges: &selected_instructions::FunctionLiveRanges,
        physical: &ValidatedPhysicalRegisterModel,
        work: &mut Work,
    ) -> Result<FunctionSpillChoices, SpillChoiceError> {
        if legality.machine != ranges.machine
            || legality.virtual_registers.len() != ranges.virtual_registers.len()
        {
            return Err(SpillChoiceError::FunctionMismatch {
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
        let mut assigned = Vec::<(VirtualRegisterId, RegisterViewId)>::new();
        for (position, start, end) in order {
            work.rule_evaluations(1)?;
            let register = &legality.virtual_registers[position];
            let range = ranges
                .virtual_registers
                .get(position)
                .filter(|range| {
                    range.virtual_register == register.virtual_register
                        && range.class == register.class
                })
                .ok_or(SpillChoiceError::VirtualRegisterMismatch {
                    function: function_index,
                    register: register.virtual_register.0,
                })?;
            if !register.entry_transitions.is_empty() {
                return Err(SpillChoiceError::UnresolvedEntryTransitions {
                    function: function_index,
                    register: register.virtual_register.0,
                });
            }
            active.retain(|entry| entry.end > start);
            let candidates = common_candidates(function_index, register)?;
            let selected = first_free(
                function_index,
                register,
                &candidates,
                &active,
                ranges,
                physical,
                None,
                &assigned,
                work,
            )?;
            if let Some(view) = selected {
                active.push(ActiveHome {
                    register: register.virtual_register,
                    class: register.class,
                    start,
                    end,
                    view,
                });
                active.sort_by_key(|entry| (entry.end, entry.register));
                assigned.push((register.virtual_register, view));
                continue;
            }

            if !ranges.early_clobbers.is_empty() {
                return Err(SpillChoiceError::UnsupportedEarlyClobber {
                    function: function_index,
                });
            }

            let block = register.spans[0].block;
            if register.spans.iter().any(|point| point.block != block) {
                return Err(SpillChoiceError::UnsupportedPressureShape {
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
                    .ok_or(SpillChoiceError::VirtualRegisterMismatch {
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
                    .is_none_or(|candidate| {
                        candidate.spans.iter().any(|point| point.block != block)
                    })
            }) {
                return Err(SpillChoiceError::UnsupportedPressureShape {
                    function: function_index,
                    register: register.virtual_register.0,
                });
            }
            let active_residents = active
                .iter()
                .map(|entry| PressureResident {
                    virtual_register: entry.register,
                    class: entry.class,
                    start: entry.start,
                    exclusive_end: entry.end,
                    view: entry.view,
                })
                .collect::<Vec<_>>();
            let mut contenders = vec![PressureContender {
                virtual_register: register.virtual_register,
                exclusive_end: end,
                reclaimed_view: None,
            }];
            for resident in &active {
                let reclaimed = first_free(
                    function_index,
                    register,
                    &candidates,
                    &active,
                    ranges,
                    physical,
                    Some(resident.register),
                    &assigned,
                    work,
                )?;
                if let Some(view) = reclaimed {
                    contenders.push(PressureContender {
                        virtual_register: resident.register,
                        exclusive_end: resident.end,
                        reclaimed_view: Some(view),
                    });
                }
            }
            contenders.sort_by_key(|contender| contender.virtual_register);
            work.candidates(count(contenders.len())?)?;
            work.commits(1)?;
            let selected_victim = contenders
                .iter()
                .max_by_key(|contender| (contender.exclusive_end, contender.virtual_register))
                .expect("incoming contender is always present")
                .virtual_register;
            return Ok(FunctionSpillChoices {
                machine: legality.machine,
                choice: Some(SpillChoice {
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
        Ok(FunctionSpillChoices {
            machine: legality.machine,
            choice: None,
        })
    }

    fn validate_local_shape(
        function: usize,
        register: VirtualRegisterId,
        start: LiveRangePoint,
        end: LiveRangePoint,
        block: selected_instructions::SelectedBlockId,
        range: &selected_instructions::VirtualLiveRange,
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

    fn interval_bounds(
        function: usize,
        register: &register_homes::VirtualRegisterAllocationLegality,
    ) -> Result<(LiveRangePoint, LiveRangePoint), SpillChoiceError> {
        let first = register
            .spans
            .first()
            .ok_or(SpillChoiceError::NoLivePoints {
                function,
                register: register.virtual_register.0,
            })?;
        let last = register.spans.last().expect("nonempty spans established");
        Ok((first.start, last.end))
    }

    fn common_candidates(
        function: usize,
        register: &register_homes::VirtualRegisterAllocationLegality,
    ) -> Result<BTreeSet<RegisterViewId>, SpillChoiceError> {
        let first = register
            .spans
            .first()
            .ok_or(SpillChoiceError::NoLivePoints {
                function,
                register: register.virtual_register.0,
            })?;
        let mut common = first.candidates.iter().copied().collect::<BTreeSet<_>>();
        for point in &register.spans[1..] {
            common.retain(|candidate| point.candidates.binary_search(candidate).is_ok());
        }
        for point in &register.early_clobber_points {
            common.retain(|candidate| point.candidates.binary_search(candidate).is_ok());
        }
        if common.is_empty() {
            return Err(SpillChoiceError::NoCommonCandidate {
                function,
                register: register.virtual_register.0,
            });
        }
        Ok(common)
    }

    fn early_clobber_conflicts(
        incoming: VirtualRegisterId,
        view: &RegisterView,
        assigned: &[(VirtualRegisterId, RegisterViewId)],
        ranges: &selected_instructions::FunctionLiveRanges,
        physical: &ValidatedPhysicalRegisterModel,
    ) -> bool {
        assigned.iter().any(|(seated, seated_view_id)| {
            let seated_view = &physical.model().views[usize::from(seated_view_id.0)];
            ranges.early_clobbers.iter().any(|early| {
                (early.def_virtual_register == incoming
                    && early
                        .uses
                        .iter()
                        .any(|used| used.virtual_register == *seated)
                    && view
                        .write_units
                        .iter()
                        .any(|unit| seated_view.units.contains(unit)))
                    || (early.def_virtual_register == *seated
                        && early
                            .uses
                            .iter()
                            .any(|used| used.virtual_register == incoming)
                        && seated_view
                            .write_units
                            .iter()
                            .any(|unit| view.units.contains(unit)))
            })
        })
    }

    fn checked_view<'a>(
        function: usize,
        register: &register_homes::VirtualRegisterAllocationLegality,
        candidate: RegisterViewId,
        physical: &'a ValidatedPhysicalRegisterModel,
    ) -> Result<&'a RegisterView, SpillChoiceError> {
        physical
            .model()
            .views
            .get(usize::from(candidate.0))
            .filter(|view| view.id == candidate && view.class == register.class)
            .ok_or(SpillChoiceError::UnknownOrIncompatibleView {
                function,
                register: register.virtual_register.0,
                view: candidate.0,
            })
    }

    #[allow(clippy::too_many_arguments)]
    fn first_free(
        function: usize,
        register: &register_homes::VirtualRegisterAllocationLegality,
        candidates: &BTreeSet<RegisterViewId>,
        active: &[ActiveHome],
        ranges: &selected_instructions::FunctionLiveRanges,
        physical: &ValidatedPhysicalRegisterModel,
        omitted: Option<VirtualRegisterId>,
        seated: &[(VirtualRegisterId, RegisterViewId)],
        work: &mut Work,
    ) -> Result<Option<RegisterViewId>, SpillChoiceError> {
        let incoming = register.virtual_register;
        for candidate in candidates {
            work.validation_steps(1)?;
            let view = checked_view(function, register, *candidate, physical)?;
            let blocked = active
                .iter()
                .filter(|entry| Some(entry.register) != omitted)
                .any(|entry| {
                    interferes(incoming, entry.register, &ranges.interference)
                        && view
                            .footprints_overlap(&physical.model().views[usize::from(entry.view.0)])
                });
            if !blocked && !early_clobber_conflicts(incoming, view, seated, ranges, physical) {
                return Ok(Some(*candidate));
            }
        }
        Ok(None)
    }

    fn interferes(
        left: VirtualRegisterId,
        right: VirtualRegisterId,
        interference: &VirtualInterferences,
    ) -> bool {
        let pair = if left < right {
            VirtualInterference {
                lower: left,
                higher: right,
            }
        } else {
            VirtualInterference {
                lower: right,
                higher: left,
            }
        };
        interference.binary_search(&pair).is_ok()
    }
}

use optimization_core::OptimizationWorkUsage;
use register_homes::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, SpillChoiceIdentity, SpillChoicePlan,
};
use selected_instructions::LiveRangeIdentity;

use crate::WorkDefect;

pub fn choose_spill_victims(
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &PreparedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    policy: SpillChoicePolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedSpillChoices, SpillChoiceError> {
    let plan = compute::compute_terminal_spill_choices(
        legality,
        ranges,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        policy,
        budget,
    )?;
    seal_spill_choices(plan)
}

seal_plan!(
    seal_spill_choices(SpillChoicePlan)
        -> ValidatedSpillChoices: SpillChoiceValidationReceipt
        = register_homes::spill_choice_identity, SpillChoiceError
    {
        legality,
        ranges,
        register_environment,
        allocator_availability,
        policy,
        usage
    }
);

copy_receipt! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SpillChoiceValidationReceipt {
        pub(crate) identity: SpillChoiceIdentity,
        pub(crate) legality: AllocationLegalityIdentity,
        pub(crate) ranges: LiveRangeIdentity,
        pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
        pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
        pub(crate) policy: SpillChoicePolicy,
        pub(crate) usage: OptimizationWorkUsage,
    }
}

validated_plan_carrier!(ValidatedSpillChoices: SpillChoicePlan, SpillChoiceValidationReceipt);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpillChoiceError {
    RootMismatch,
    UnsupportedPolicy,
    UnsupportedTiedOperands {
        function: usize,
    },
    UnsupportedEarlyClobber {
        function: usize,
    },
    Work(WorkDefect),
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
    UnsupportedPressureShape {
        function: usize,
        register: u32,
    },
    ChoiceMismatch {
        function: usize,
    },
    UsageMismatch,
}

failed_display!(SpillChoiceError, "Terminal recovery-victim choice failed");

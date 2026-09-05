//! Independently keyed reconstruction of fixed/precolored point intervals.

use std::collections::BTreeMap;

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::RegisterOperandAccess;

use crate::{
    FixedPrecoloredInterval, FixedPrecoloredIntervalError, FixedPrecoloredIntervalPolicy,
    FunctionAllocationLegality, FunctionFixedPrecoloredIntervals, FunctionLiveRanges,
    LiveRangePoint, ValidatedAllocationLegality, ValidatedLiveRanges, VirtualFixedConstraintSite,
};

pub(super) struct ReplayedIntervals {
    pub(super) functions: Vec<FunctionFixedPrecoloredIntervals>,
    pub(super) structural_unit_functions: Vec<FunctionFixedPrecoloredIntervals>,
    pub(super) usage: OptimizationWorkUsage,
}

pub(super) fn replay(
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    policy: FixedPrecoloredIntervalPolicy,
    budget: OptimizationWorkBudget,
) -> Result<ReplayedIntervals, FixedPrecoloredIntervalError> {
    match policy {
        FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1 => {}
    }
    let mut usage = OptimizationWorkUsage {
        iterations: 2,
        ..Default::default()
    };
    let functions = replay_family(
        &ranges.plan().functions,
        &legality.plan().functions,
        &mut usage,
    )?;
    let structural_unit_functions = replay_family(
        &ranges.plan().structural_unit_functions,
        &legality.plan().structural_unit_functions,
        &mut usage,
    )?;
    if !usage.within(budget) {
        return Err(FixedPrecoloredIntervalError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    Ok(ReplayedIntervals {
        functions,
        structural_unit_functions,
        usage,
    })
}

fn replay_family(
    ranges: &[FunctionLiveRanges],
    legality: &[FunctionAllocationLegality],
    usage: &mut OptimizationWorkUsage,
) -> Result<Vec<FunctionFixedPrecoloredIntervals>, FixedPrecoloredIntervalError> {
    if ranges.len() != legality.len() {
        return Err(FixedPrecoloredIntervalError::RootMismatch);
    }
    ranges
        .iter()
        .enumerate()
        .map(|(function, ranges)| replay_function(function, ranges, &legality[function], usage))
        .collect()
}

fn replay_function(
    function: usize,
    ranges: &FunctionLiveRanges,
    legality: &FunctionAllocationLegality,
    usage: &mut OptimizationWorkUsage,
) -> Result<FunctionFixedPrecoloredIntervals, FixedPrecoloredIntervalError> {
    add(&mut usage.rule_evaluations, 1)?;
    if ranges.machine != legality.machine {
        return Err(FixedPrecoloredIntervalError::FunctionMismatch { function });
    }
    let mut legal_by_register = BTreeMap::new();
    for legal in &legality.virtual_registers {
        if legal_by_register
            .insert(legal.virtual_register, legal)
            .is_some()
        {
            return Err(FixedPrecoloredIntervalError::RegisterMismatch {
                function,
                register: legal.virtual_register.0,
            });
        }
    }
    if legal_by_register.len() != ranges.virtual_registers.len() {
        return Err(FixedPrecoloredIntervalError::FunctionMismatch { function });
    }
    let mut intervals = Vec::new();
    for range in &ranges.virtual_registers {
        add(&mut usage.validation_steps, 1)?;
        let legal = legal_by_register.remove(&range.virtual_register).ok_or(
            FixedPrecoloredIntervalError::RegisterMismatch {
                function,
                register: range.virtual_register.0,
            },
        )?;
        if legal.class != range.class {
            return Err(FixedPrecoloredIntervalError::RegisterMismatch {
                function,
                register: range.virtual_register.0,
            });
        }
        let point_rows = point_index(function, range, legal)?;
        for constraint in &range.fixed_constraints {
            replay_early_clobber_refusal(function, ranges, range, constraint.site)?;
            add(&mut usage.candidates, 1)?;
            add(&mut usage.validation_steps, 1)?;
            let point = match constraint.site {
                VirtualFixedConstraintSite::Entry => range.fragments.first().map(|row| row.start),
                VirtualFixedConstraintSite::Operand { point, .. } => Some(point),
            }
            .ok_or(FixedPrecoloredIntervalError::ConstraintPointMissing {
                function,
                register: range.virtual_register.0,
                point: 0,
            })?;
            let (block, candidates) = point_rows.get(&point).ok_or(
                FixedPrecoloredIntervalError::ConstraintPointMissing {
                    function,
                    register: range.virtual_register.0,
                    point: point.0,
                },
            )?;
            if candidates.as_slice() != [constraint.view] {
                return Err(FixedPrecoloredIntervalError::ConstraintViewMismatch {
                    function,
                    register: range.virtual_register.0,
                    view: constraint.view.0,
                });
            }
            let end = LiveRangePoint(point.0.checked_add(1).ok_or(
                FixedPrecoloredIntervalError::IntervalOverflow {
                    function,
                    register: range.virtual_register.0,
                    point: point.0,
                },
            )?);
            intervals.push(FixedPrecoloredInterval {
                virtual_register: range.virtual_register,
                class: range.class,
                site: constraint.site,
                block: *block,
                start: point,
                end,
                view: constraint.view,
            });
            add(&mut usage.commits, 1)?;
        }
    }
    if !legal_by_register.is_empty() {
        return Err(FixedPrecoloredIntervalError::FunctionMismatch { function });
    }
    intervals.sort_by_key(replay_key);
    Ok(FunctionFixedPrecoloredIntervals {
        machine: ranges.machine,
        intervals,
    })
}

fn replay_early_clobber_refusal(
    function: usize,
    ranges: &FunctionLiveRanges,
    range: &crate::VirtualLiveRange,
    site: VirtualFixedConstraintSite,
) -> Result<(), FixedPrecoloredIntervalError> {
    let VirtualFixedConstraintSite::Operand {
        instruction,
        operand,
        access: RegisterOperandAccess::Def,
        ..
    } = site
    else {
        return Ok(());
    };
    let rows = ranges
        .early_clobbers
        .iter()
        .filter(|row| row.def_virtual_register == range.virtual_register)
        .filter(|row| row.instruction == instruction && row.def_operand == operand)
        .count();
    if rows != 0 {
        return Err(
            FixedPrecoloredIntervalError::UnsupportedEarlyClobberFixedConstraint {
                function,
                register: range.virtual_register.0,
                instruction: instruction.0,
                operand,
            },
        );
    }
    Ok(())
}

fn point_index(
    function: usize,
    range: &crate::VirtualLiveRange,
    legal: &crate::VirtualRegisterAllocationLegality,
) -> Result<
    BTreeMap<
        LiveRangePoint,
        (
            selected_instructions::SelectedBlockId,
            Vec<register_model::RegisterViewId>,
        ),
    >,
    FixedPrecoloredIntervalError,
> {
    let mut points = BTreeMap::new();
    for row in &legal.points {
        if points
            .insert(row.point, (row.block, row.candidates.clone()))
            .is_some()
        {
            return Err(FixedPrecoloredIntervalError::ConstraintPointMissing {
                function,
                register: range.virtual_register.0,
                point: row.point.0,
            });
        }
    }
    for fragment in &range.fragments {
        for raw in fragment.start.0..fragment.end.0 {
            let point = LiveRangePoint(raw);
            if points.get(&point).map(|row| row.0) != Some(fragment.block) {
                return Err(FixedPrecoloredIntervalError::ConstraintPointMissing {
                    function,
                    register: range.virtual_register.0,
                    point: point.0,
                });
            }
        }
    }
    Ok(points)
}

fn replay_key(row: &FixedPrecoloredInterval) -> (u32, u8, u32, u32, u16, u8) {
    match row.site {
        VirtualFixedConstraintSite::Entry => (row.virtual_register.0, 0, 0, 0, 0, 0),
        VirtualFixedConstraintSite::Operand {
            point,
            instruction,
            operand,
            access,
            ..
        } => (
            row.virtual_register.0,
            1,
            point.0,
            instruction.0,
            operand,
            match access {
                RegisterOperandAccess::Use => 0,
                RegisterOperandAccess::Def => 1,
                RegisterOperandAccess::UseDef => 2,
            },
        ),
    }
}

fn add(target: &mut u64, amount: u64) -> Result<(), FixedPrecoloredIntervalError> {
    *target = target
        .checked_add(amount)
        .ok_or(FixedPrecoloredIntervalError::WorkOverflow)?;
    Ok(())
}

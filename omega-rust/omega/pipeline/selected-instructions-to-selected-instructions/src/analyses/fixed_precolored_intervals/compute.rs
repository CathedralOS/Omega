//! Canonical direct traversal for fixed/precolored point intervals.

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::{
    FixedPrecoloredInterval, FixedPrecoloredIntervalError, FixedPrecoloredIntervalPlan,
    FixedPrecoloredIntervalPolicy, FunctionAllocationLegality, FunctionFixedPrecoloredIntervals,
    FunctionLiveRanges, LiveRangePoint, ValidatedAllocationLegality, ValidatedLiveRanges,
    VirtualFixedConstraintSite,
};

pub(super) fn compute(
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    policy: FixedPrecoloredIntervalPolicy,
    budget: OptimizationWorkBudget,
) -> Result<FixedPrecoloredIntervalPlan, FixedPrecoloredIntervalError> {
    roots(ranges, legality)?;
    let mut usage = OptimizationWorkUsage {
        iterations: 2,
        ..Default::default()
    };
    let functions = family(
        &ranges.plan().functions,
        &legality.plan().functions,
        &mut usage,
    )?;
    let structural_unit_functions = family(
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
    Ok(FixedPrecoloredIntervalPlan {
        ranges: ranges.receipt().identity(),
        legality: legality.receipt().identity(),
        register_environment: legality.receipt().register_environment(),
        allocator_availability: legality.receipt().allocator_availability(),
        optimization_unit: ranges.receipt().optimization_unit(),
        fuel_schedule: ranges.receipt().fuel_schedule(),
        policy,
        budget,
        usage,
        functions,
        structural_unit_functions,
    })
}

fn roots(
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
) -> Result<(), FixedPrecoloredIntervalError> {
    if legality.receipt().ranges() != ranges.receipt().identity()
        || ranges.plan().functions.len() != legality.plan().functions.len()
        || ranges.plan().structural_unit_functions.len()
            != legality.plan().structural_unit_functions.len()
    {
        return Err(FixedPrecoloredIntervalError::RootMismatch);
    }
    Ok(())
}

fn family(
    ranges: &[FunctionLiveRanges],
    legality: &[FunctionAllocationLegality],
    usage: &mut OptimizationWorkUsage,
) -> Result<Vec<FunctionFixedPrecoloredIntervals>, FixedPrecoloredIntervalError> {
    ranges
        .iter()
        .zip(legality)
        .enumerate()
        .map(|(function, (ranges, legality))| derive_function(function, ranges, legality, usage))
        .collect()
}

fn derive_function(
    function: usize,
    ranges: &FunctionLiveRanges,
    legality: &FunctionAllocationLegality,
    usage: &mut OptimizationWorkUsage,
) -> Result<FunctionFixedPrecoloredIntervals, FixedPrecoloredIntervalError> {
    add(&mut usage.rule_evaluations, 1)?;
    if ranges.machine != legality.machine
        || ranges.virtual_registers.len() != legality.virtual_registers.len()
    {
        return Err(FixedPrecoloredIntervalError::FunctionMismatch { function });
    }
    let mut intervals = Vec::new();
    for (range, legal) in ranges
        .virtual_registers
        .iter()
        .zip(&legality.virtual_registers)
    {
        add(&mut usage.validation_steps, 1)?;
        if range.virtual_register != legal.virtual_register || range.class != legal.class {
            return Err(FixedPrecoloredIntervalError::RegisterMismatch {
                function,
                register: range.virtual_register.0,
            });
        }
        for constraint in &range.fixed_constraints {
            reject_early_clobber_fixed(function, ranges, range, constraint.site)?;
            add(&mut usage.candidates, 1)?;
            add(&mut usage.validation_steps, 1)?;
            let (block, start) = resolve_point(function, range, constraint.site)?;
            require_view(
                function,
                range.virtual_register.0,
                block,
                start,
                constraint.view,
                legal,
            )?;
            let end = LiveRangePoint(start.0.checked_add(1).ok_or(
                FixedPrecoloredIntervalError::IntervalOverflow {
                    function,
                    register: range.virtual_register.0,
                    point: start.0,
                },
            )?);
            intervals.push(FixedPrecoloredInterval {
                virtual_register: range.virtual_register,
                class: range.class,
                site: constraint.site,
                block,
                start,
                end,
                view: constraint.view,
            });
            add(&mut usage.commits, 1)?;
        }
    }
    intervals.sort_by_key(interval_key);
    Ok(FunctionFixedPrecoloredIntervals {
        machine: ranges.machine,
        intervals,
    })
}

fn reject_early_clobber_fixed(
    function: usize,
    ranges: &FunctionLiveRanges,
    range: &crate::VirtualLiveRange,
    site: VirtualFixedConstraintSite,
) -> Result<(), FixedPrecoloredIntervalError> {
    let VirtualFixedConstraintSite::Operand {
        instruction,
        operand,
        access: register_model::RegisterOperandAccess::Def,
        ..
    } = site
    else {
        return Ok(());
    };
    if ranges.early_clobbers.iter().any(|row| {
        row.instruction == instruction
            && row.def_operand == operand
            && row.def_virtual_register == range.virtual_register
    }) {
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

fn resolve_point(
    function: usize,
    range: &crate::VirtualLiveRange,
    site: VirtualFixedConstraintSite,
) -> Result<(selected_instructions::SelectedBlockId, LiveRangePoint), FixedPrecoloredIntervalError>
{
    let point = match site {
        VirtualFixedConstraintSite::Entry => range.fragments.first().map(|row| row.start),
        VirtualFixedConstraintSite::Operand { point, .. } => Some(point),
    }
    .ok_or(FixedPrecoloredIntervalError::ConstraintPointMissing {
        function,
        register: range.virtual_register.0,
        point: 0,
    })?;
    let blocks = range
        .fragments
        .iter()
        .filter(|row| row.start <= point && point < row.end)
        .map(|row| row.block)
        .collect::<Vec<_>>();
    if blocks.len() != 1 {
        return Err(FixedPrecoloredIntervalError::ConstraintPointMissing {
            function,
            register: range.virtual_register.0,
            point: point.0,
        });
    }
    Ok((blocks[0], point))
}

fn require_view(
    function: usize,
    register: u32,
    block: selected_instructions::SelectedBlockId,
    point: LiveRangePoint,
    view: register_model::RegisterViewId,
    legality: &crate::VirtualRegisterAllocationLegality,
) -> Result<(), FixedPrecoloredIntervalError> {
    let rows = legality
        .points
        .iter()
        .filter(|row| row.block == block && row.point == point)
        .collect::<Vec<_>>();
    if rows.len() != 1 {
        return Err(FixedPrecoloredIntervalError::ConstraintPointMissing {
            function,
            register,
            point: point.0,
        });
    }
    if rows[0].candidates.as_slice() != [view] {
        return Err(FixedPrecoloredIntervalError::ConstraintViewMismatch {
            function,
            register,
            view: view.0,
        });
    }
    Ok(())
}

fn interval_key(row: &FixedPrecoloredInterval) -> (u32, u8, u32, u32, u16, u8) {
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
            access_key(access),
        ),
    }
}

fn access_key(access: register_model::RegisterOperandAccess) -> u8 {
    match access {
        register_model::RegisterOperandAccess::Use => 0,
        register_model::RegisterOperandAccess::Def => 1,
        register_model::RegisterOperandAccess::UseDef => 2,
    }
}

fn add(target: &mut u64, amount: u64) -> Result<(), FixedPrecoloredIntervalError> {
    *target = target
        .checked_add(amount)
        .ok_or(FixedPrecoloredIntervalError::WorkOverflow)?;
    Ok(())
}

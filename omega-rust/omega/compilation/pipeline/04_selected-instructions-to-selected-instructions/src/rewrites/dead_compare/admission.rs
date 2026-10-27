//! Shared admission for dead-compare removal: locate the named compare,
//! confirm its canonical flag-publisher shape against the target's own
//! constraint row, and prove every condition-state unit it publishes dies
//! unread on every forward path.
//!
//! The compare kinds are the only instructions whose complete observable
//! effect is the implicit flag surface they publish — every other kind
//! carries a scalar result, a memory effect, or a control transfer that a
//! removal would silently drop. Once each published unit is proven dead —
//! killed by a redefinition or clobber, or carried out of the function,
//! before any implicit use can observe it — the instruction computes
//! nothing and leaves the plan.
//!
//! This file also owns the helpers the sibling removal families reuse:
//! `canonical_compare_operands` reads the emitted flag-publisher shape any
//! compare — victim or shadow — must carry, `admit_shadowed_compare` is the
//! cone-and-reaching-set admission both shadow-based families run — only
//! their shadow agreement rule and measured-step term differ —
//! `audit_stable_paths` is the backward operand-stability walk the
//! shadow-based families run per published unit, and `compare_removal_steps`
//! is the cone-audited measured-step contract both shadow-based families
//! publish.
use std::collections::BTreeSet;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use register_model::RegisterUnitId;
use selected_instructions::{
    SelectedBlockId, SelectedCasePayloadTransport, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedValueTransport, VirtualRegisterId,
};

use super::DeadCompareError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    block_instructions, edge_surface, member_position, terminator_successors,
};
use crate::rewrites::condition_state::{
    EventSite, adjacency, backward_cone, entry_index, instruction_at, reaching_events,
};

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    pub block: SelectedBlockId,
    pub compare_index: usize,
}

pub(super) fn compare_operand_arity(kind: SelectedInstructionKind) -> Option<usize> {
    match kind {
        SelectedInstructionKind::CompareI64 => Some(2),
        SelectedInstructionKind::CompareI64Immediate { .. }
        | SelectedInstructionKind::CompareI64Zero => Some(1),
        _ => None,
    }
}

pub(super) fn canonical_compare_operands(
    instruction: &SelectedInstruction,
) -> Option<Vec<VirtualRegisterId>> {
    if compare_operand_arity(instruction.kind)? != instruction.operands.len()
        || !instruction.implicit_uses.is_empty()
        || instruction
            .implicit_defs
            .iter()
            .chain(instruction.clobbers.iter())
            .next()
            .is_none()
        || instruction
            .operands
            .iter()
            .enumerate()
            .any(|(position, operand)| {
                operand.operand != position as u16
                    || operand.access != RegisterOperandAccess::Use
                    || operand.fixed_view.is_some()
                    || operand.tied_to.is_some()
                    || operand.early_clobber
            })
    {
        return None;
    }
    Some(
        instruction
            .operands
            .iter()
            .map(|operand| operand.virtual_register)
            .collect(),
    )
}

pub(super) fn audit_stable_paths<E: Copy>(
    function: &SelectedFunction,
    predecessors: &[Vec<usize>],
    block_index: usize,
    compare_index: usize,
    unit_sites: &BTreeSet<EventSite>,
    registers: &BTreeSet<VirtualRegisterId>,
    violation: E,
) -> Result<(), E> {
    let compare_site = (block_index, compare_index);
    let mut visited = BTreeSet::new();
    let mut frontier = vec![compare_site];
    while let Some(site @ (block, position)) = frontier.pop() {
        if !visited.insert(site) {
            continue;
        }
        // The compare's own position seeds the walk and ends cyclic
        // encounters; every other resolved shadow for the unit blocks the
        // exposed interval from reaching behind it.
        if site != compare_site && unit_sites.contains(&site) {
            continue;
        }
        let instruction = instruction_at(function, site);
        if instruction.operands.iter().any(|operand| {
            registers.contains(&operand.virtual_register)
                && (operand.access != RegisterOperandAccess::Use || operand.early_clobber)
        }) {
            return Err(violation);
        }
        if position > 0 {
            frontier.push((block, position - 1));
            continue;
        }
        for &predecessor in &predecessors[block] {
            for successor in terminator_successors(&function.blocks[predecessor].terminator) {
                if successor.block != function.blocks[block].id {
                    continue;
                }
                for binding in &successor.bindings {
                    if let SelectedValueTransport::Registers { parameter, .. } = binding.transport
                        && registers.contains(&parameter)
                    {
                        return Err(violation);
                    }
                }
                if let Some(case) = &successor.structural_case {
                    for payload in &case.payloads {
                        let parameter = match payload.transport {
                            SelectedCasePayloadTransport::Unused => continue,
                            SelectedCasePayloadTransport::Unmaterialized { parameter }
                            | SelectedCasePayloadTransport::Registers { parameter, .. } => {
                                parameter
                            }
                        };
                        if registers.contains(&parameter) {
                            return Err(violation);
                        }
                    }
                }
            }
            frontier.push((predecessor, function.blocks[predecessor].instructions.len()));
        }
    }
    Ok(())
}

/// The refusal surface one shadow-based compare family maps the shared
/// admission walk's failures onto, so the walk never names another family's
/// error type: its source, shape, constraint, use, overflow and budget
/// refusals.
pub(super) struct ShadowRefusals<E> {
    pub source: E,
    pub unsupported_instruction: E,
    pub constraint: E,
    pub unsupported_use: E,
    pub overflow: E,
    pub budget: E,
}

/// What one family's own canonical-shape read established for the victim
/// compare: the operand registers every reaching-site rule compares against,
/// and the family's own read of it — the strict family's flag-equivalent form
/// is those registers alone, while the equivalent family keeps the
/// subtraction sides its rule proves equal site by site.
pub(super) struct ShadowVictim<V> {
    pub registers: Vec<VirtualRegisterId>,
    pub read: V,
}

/// One family's reaching-site agreement rule: whether a site is one the
/// family accepts for the unit, recording the registers the backward operand
/// audit must then hold stable.
type ShadowAgreement<V> = fn(
    &SelectedFunction,
    &SelectedInstruction,
    &ShadowVictim<V>,
    &SelectedInstruction,
    &mut BTreeSet<VirtualRegisterId>,
) -> bool;

/// The admission every compare removal that proves its published units
/// carried by reaching shadow sites runs: locate the named compare, take the
/// family's own read of its canonical flag-publisher shape, agree its
/// constraint row, and resolve each published unit's reaching events to the
/// sites the family's agreement rule accepts. A defined unit's accepted sites
/// bound the backward operand audit over the registers the rule recorded as
/// shared; a clobbered unit's sites must leave it unspecified. The cone, the
/// reaching walk and the budget charge are what both shadow-based families
/// share: each supplies its victim read, its agreement rule and its
/// measured-step term, and nothing else differs.
pub(super) fn admit_shadowed_compare<'source, E: Copy, V>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    refusals: ShadowRefusals<E>,
    victim_read: impl FnOnce(&SelectedInstruction) -> Option<ShadowVictim<V>>,
    agreement: ShadowAgreement<V>,
    per_unit: impl FnOnce(&crate::rewrites::ConePropagation) -> Result<usize, E>,
) -> Result<Admission<'source>, E> {
    let (plan, function) =
        crate::rewrites::plan_function(source, function_index, environment, || refusals.source)?;
    let (block_index, compare_index) = member_position(function, compare).ok_or(refusals.source)?;
    let block = &function.blocks[block_index];
    let compare_instruction = &block.instructions[compare_index];
    let victim = victim_read(compare_instruction).ok_or(refusals.unsupported_instruction)?;
    let arity = compare_instruction.operands.len();
    agree_compare_removal_row(
        function,
        compare,
        compare_instruction,
        arity,
        environment,
        || refusals.constraint,
        || refusals.unsupported_instruction,
    )?;
    // The flag walk crosses block boundaries: the adjacency, the entry whose
    // unseeded condition state bounds every path, and the cone of blocks whose
    // entry sets can feed the reaching events are what both families consult.
    let (successors, predecessors) = adjacency(function);
    let entry = entry_index(function).ok_or(refusals.source)?;
    let cone = backward_cone(&predecessors, block_index);
    let units: BTreeSet<RegisterUnitId> = compare_instruction
        .implicit_defs
        .iter()
        .chain(compare_instruction.clobbers.iter())
        .copied()
        .collect();
    for &unit in &units {
        let defined = compare_instruction.implicit_defs.contains(&unit);
        let sites = reaching_events(
            function,
            entry,
            &successors,
            &cone,
            block_index,
            compare_index,
            unit,
        )
        .map_err(|_| refusals.unsupported_use)?;
        let mut unit_sites = BTreeSet::new();
        let mut audited = BTreeSet::new();
        for site in sites {
            let event = instruction_at(function, site);
            if defined {
                if !event.implicit_defs.contains(&unit)
                    || event.clobbers.contains(&unit)
                    || !agreement(function, compare_instruction, &victim, event, &mut audited)
                {
                    return Err(refusals.unsupported_use);
                }
                unit_sites.insert(site);
            } else if !event.clobbers.contains(&unit) || event.implicit_defs.contains(&unit) {
                return Err(refusals.unsupported_use);
            }
        }
        if defined {
            audit_stable_paths(
                function,
                &predecessors,
                block_index,
                compare_index,
                &unit_sites,
                &audited,
                refusals.unsupported_use,
            )?;
        }
    }
    let steps = compare_removal_steps(plan, function, &successors, units.len(), per_unit, || {
        refusals.overflow
    })?;
    crate::rewrites::charge_steps(steps, budget, || refusals.overflow, || refusals.budget)?;
    Ok(Admission {
        function,
        block_index,
        block: block.id,
        compare_index,
    })
}

/// The cone-audited step contract the backward-walk compare removals
/// publish: the cone-propagation bases, the shadow-transport rows between
/// successor edges, the removal audit at the function scan, edge count, and
/// transport rows per published unit, and the shared rewrite, roster, call,
/// and access scans — plus the caller's own per-unit term. Only the per-unit
/// term differs between the shadow-based families, so they derive their
/// measured steps here rather than restating the audit.
pub(super) fn compare_removal_steps<E>(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
    successors: &[Vec<usize>],
    units: usize,
    per_unit: impl FnOnce(&crate::rewrites::ConePropagation) -> Result<usize, E>,
    overflow: impl Fn() -> E + Copy,
) -> Result<usize, E> {
    let cone = crate::rewrites::cone_propagation(function, successors, overflow)?;
    let function_scan = cone.function_scan;
    let edge_count = cone.edge_count;
    let per_unit = per_unit(&cone)?;
    let walk_setup = cone.walk_setup_base;
    let transport_rows = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            terminator_successors(&block.terminator)
                .iter()
                .try_fold(total, |total, successor| {
                    total.checked_add(edge_surface(successor))
                })
        })
        .ok_or_else(overflow)?;
    let audit = units
        .checked_mul(
            function_scan
                .checked_add(edge_count)
                .and_then(|total| total.checked_add(transport_rows))
                .ok_or_else(overflow)?,
        )
        .ok_or_else(overflow)?;
    crate::rewrites::rewrite_scan(plan, function)
        .and_then(|total| {
            total
                .checked_add(function_scan)?
                .checked_add(function.virtual_registers.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.memory_accesses.len())
        })
        .and_then(|total| {
            total
                .checked_add(walk_setup)?
                .checked_add(units.checked_mul(per_unit)?)?
                .checked_add(audit)
        })
        .ok_or_else(overflow)
}

fn audit_dead_unit(
    function: &SelectedFunction,
    block_index: usize,
    compare_index: usize,
    unit: RegisterUnitId,
) -> Result<(), DeadCompareError> {
    let mut visited = BTreeSet::new();
    let mut frontier = vec![(block_index, compare_index + 1)];
    while let Some((current, start)) = frontier.pop() {
        if !visited.insert((current, start)) {
            continue;
        }
        let block = &function.blocks[current];
        let mut killed = false;
        for instruction in block_instructions(block).skip(start) {
            if instruction.implicit_uses.contains(&unit) {
                return Err(DeadCompareError::UnsupportedUse);
            }
            if instruction.implicit_defs.contains(&unit) || instruction.clobbers.contains(&unit) {
                killed = true;
                break;
            }
        }
        if killed {
            continue;
        }
        for successor in terminator_successors(&block.terminator) {
            let target = function
                .block_index(successor.block)
                .ok_or(DeadCompareError::UnsupportedUse)?;
            frontier.push((target, 0));
        }
    }
    Ok(())
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, DeadCompareError> {
    let (plan, function) =
        crate::rewrites::plan_function(source, function_index, environment, || {
            DeadCompareError::SourceMismatch
        })?;
    let (block_index, compare_index) =
        member_position(function, compare).ok_or(DeadCompareError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let compare_instruction = &block.instructions[compare_index];
    // The removed instruction must be one of the three compare forms in the
    // emitted flag-publisher shape: every operand a plain `Use` at its
    // canonical position, no implicit uses the removal would silently drop,
    // and a nonempty published surface — a compare defining no condition
    // state at all is malformed, not dead.
    let arity = canonical_compare_operands(compare_instruction)
        .ok_or(DeadCompareError::UnsupportedInstruction)?
        .len();
    agree_compare_removal_row(
        function,
        compare,
        compare_instruction,
        arity,
        environment,
        || DeadCompareError::ConstraintMismatch,
        || DeadCompareError::UnsupportedInstruction,
    )?;
    // Every condition-state unit the compare publishes must die unread:
    // each walks forward from the compare until a redefinition or clobber
    // kills it on every path, refusing the first reached reader.
    let units: BTreeSet<RegisterUnitId> = compare_instruction
        .implicit_defs
        .iter()
        .chain(compare_instruction.clobbers.iter())
        .copied()
        .collect();
    for &unit in &units {
        audit_dead_unit(function, block_index, compare_index, unit)?;
    }
    let function_scan =
        crate::rewrites::function_scan(function).ok_or(DeadCompareError::IdentityOverflow)?;
    // The dead-unit audit visits each block once per entry position per
    // published unit. Only the compare's own block carries a nonzero start,
    // so two visits per block per unit bound the scan; each visit walks the
    // body's instructions and terminator plus, when the unit stays live,
    // the successor edges.
    let edge_count = function
        .blocks
        .iter()
        .map(|block| terminator_successors(&block.terminator).len())
        .sum::<usize>();
    let dead_audit = units
        .len()
        .checked_mul(
            function_scan
                .checked_mul(2)
                .and_then(|scan| scan.checked_add(edge_count))
                .ok_or(DeadCompareError::IdentityOverflow)?,
        )
        .ok_or(DeadCompareError::IdentityOverflow)?;
    let steps = crate::rewrites::rewrite_scan(plan, function)
        .and_then(|total| {
            // A second pass of this function audits the operand classes,
            // roster rows, and call/access rows the compare must not be
            // named by.
            total
                .checked_add(function_scan)?
                .checked_add(function.virtual_registers.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.memory_accesses.len())
        })
        .and_then(|total| total.checked_add(dead_audit))
        .ok_or(DeadCompareError::IdentityOverflow)?;
    crate::rewrites::charge_steps(
        steps,
        budget,
        || DeadCompareError::IdentityOverflow,
        || DeadCompareError::WorkBudgetExceeded,
    )?;
    Ok(Admission {
        function,
        block_index,
        block: block.id,
        compare_index,
    })
}

pub(super) fn apply<E>(
    admitted: &Admission<'_>,
    function: &mut SelectedFunction,
    map_error: impl Fn(DeadCompareError) -> E,
) -> Result<(), E> {
    let block = function
        .blocks
        .get_mut(admitted.block_index)
        .ok_or_else(|| map_error(DeadCompareError::SourceMismatch))?;
    if block.id != admitted.block {
        return Err(map_error(DeadCompareError::SourceMismatch));
    }
    function.boundary_settlements = super::super::shifted_boundary_settlements(
        admitted.function,
        admitted.block,
        admitted.compare_index,
        || DeadCompareError::SourceMismatch,
        || DeadCompareError::IdentityOverflow,
    )
    .map_err(map_error)?;
    block.instructions.remove(admitted.compare_index);
    Ok(())
}

/// Shared remove-one-compare orchestration: admit through the family's
/// `admit`, apply the admitted removal into a cloned plan with the family's
/// error map, then seal through the family's `seal`.
pub(super) fn remove_compare<'source, S, E, V>(
    source: &'source S,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    admit: fn(
        &'source S,
        usize,
        SelectedInstructionId,
        &'source ValidatedTargetRegisterEnvironment,
        OptimizationWorkBudget,
    ) -> Result<Admission<'source>, E>,
    map_error: impl Fn(DeadCompareError) -> E,
    seal: impl Fn(&'source S, usize, SelectedInstructionId, SelectedInstructionPlan) -> Result<V, E>,
) -> Result<V, E>
where
    S: ValidatedSelectedAnalysis + ?Sized,
{
    let admitted = admit(source, function_index, compare, environment, budget)?;
    let mut transformed = source.selected_plan().clone();
    apply(
        &admitted,
        &mut transformed.functions[function_index],
        map_error,
    )?;
    seal(source, function_index, compare, transformed)
}

/// The compare-removal row agreement every compare family re-proves before its
/// own audit: the compare's constraint row must declare the same all-`use`
/// operand shape at the classes the roster assigns and the same implicit
/// surface the instruction carries — a row that disagrees would change what
/// removing the instruction means — and nothing may bind the removed
/// instruction by identity, because a call contract or memory-access row
/// naming it is orphaned by the removal. `arity` is the operand count the
/// family's own canonical-shape read established.
pub(crate) fn agree_compare_removal_row<E>(
    function: &SelectedFunction,
    compare: SelectedInstructionId,
    compare_instruction: &SelectedInstruction,
    arity: usize,
    environment: &ValidatedTargetRegisterEnvironment,
    constraint_mismatch: impl Fn() -> E,
    unsupported: impl Fn() -> E,
) -> Result<(), E> {
    let row = environment
        .constraint(compare_instruction.constraint)
        .ok_or_else(&constraint_mismatch)?;
    if row.operands.len() != arity
        || row.implicit_uses != compare_instruction.implicit_uses
        || row.implicit_defs != compare_instruction.implicit_defs
        || row.clobbers != compare_instruction.clobbers
    {
        return Err(constraint_mismatch());
    }
    for (position, operand) in compare_instruction.operands.iter().enumerate() {
        let register = function
            .virtual_register(operand.virtual_register)
            .ok_or_else(&constraint_mismatch)?;
        if row.operands[position].operand != operand.operand
            || row.operands[position].access != RegisterOperandAccess::Use
            || row.operands[position].class != register.class
        {
            return Err(constraint_mismatch());
        }
    }
    if function.call(compare).is_some() || function.memory_access(compare).is_some() {
        return Err(unsupported());
    }
    Ok(())
}

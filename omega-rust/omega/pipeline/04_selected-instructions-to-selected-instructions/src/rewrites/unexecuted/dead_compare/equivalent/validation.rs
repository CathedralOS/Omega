//! Independent validation of equivalent-compare removal.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! removal's legality from the source records — the named instruction's
//! compare kind and canonical flag-publisher shape against the bound
//! constraint row, the call and memory-access rosters that must not name
//! it, the shared reaching walk resolving every published unit's last
//! observed condition-state events, and the side-by-side proof that each
//! shadow site computes the victim's own subtraction — then the backward
//! walk refusing any write to an operand register the two compares share
//! by identity. From that record it rebuilds the function the contract
//! demands and requires the proposal to equal it, and restoring the
//! removed compare with the source settlements must reproduce the
//! complete source by content. A producer admission error therefore
//! fails validation even when the proposal is exactly what that producer
//! emitted.
use std::collections::BTreeSet;
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use register_model::RegisterUnitId;
use selected_instructions::{
    SelectedBlockId, SelectedCasePayloadTransport, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedValueTransport, VirtualRegisterId,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{EquivalentCompareError, EquivalentCompareReceipt, ValidatedEquivalentCompare};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{edge_surface, terminator_successors};
use crate::rewrites::condition_state::{
    EventSite, adjacency, backward_cone, entry_index, immediate_bits, instruction_at,
    materialized_bits, reaching_events,
};

/// The validator's own reconstruction of the removal the contract
/// permits: the admitted compare's coordinates, its block's identity,
/// the successor adjacency the reaching walk built, and the published
/// unit count for the measured-step contract. It shares no state with
/// the producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    function_index: usize,
    block_index: usize,
    block: SelectedBlockId,
    compare_index: usize,
    successors: Vec<Vec<usize>>,
    /// The distinct condition-state units the compare publishes, collected
    /// by the validator's own audit for the measured-step contract.
    units: usize,
}

/// The semantic right side a canonical compare subtracts: the second
/// register operand for `CompareI64`, or the literal the form encodes —
/// `CompareI64Zero` reads as `left - 0`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CompareRight {
    Register(VirtualRegisterId),
    Literal(u64),
}

/// The `(left, right)` value pair whose subtraction a canonical flag
/// publisher's flag publication describes — the validator's own read of
/// the emitted shape, decided from the instruction record alone. The
/// left side is always a register operand; the right is a register for
/// `CompareI64` and the encoded literal for `CompareI64Immediate` and
/// `CompareI64Zero`. An instruction outside the emitted shape — wrong
/// arity for its kind, an implicit use, an empty published surface, an
/// operand that is not a plain `Use` at its canonical position, or an
/// immediate payload outside the unsigned encoding the form admits — is
/// not even a candidate.
fn compare_sides(instruction: &SelectedInstruction) -> Option<(VirtualRegisterId, CompareRight)> {
    let arity = match instruction.kind {
        SelectedInstructionKind::CompareI64 => 2,
        SelectedInstructionKind::CompareI64Immediate { .. }
        | SelectedInstructionKind::CompareI64Zero => 1,
        _ => return None,
    };
    if instruction.operands.len() != arity
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
    let left = instruction.operands[0].virtual_register;
    match instruction.kind {
        SelectedInstructionKind::CompareI64 => Some((
            left,
            CompareRight::Register(instruction.operands[1].virtual_register),
        )),
        SelectedInstructionKind::CompareI64Immediate { immediate } => {
            Some((left, CompareRight::Literal(immediate_bits(immediate)?)))
        }
        SelectedInstructionKind::CompareI64Zero => Some((left, CompareRight::Literal(0))),
        _ => None,
    }
}

/// The bit pattern one right side provably reads: a literal carries its
/// own bits, and a register resolves through its unique `MaterializeI64`
/// producer — a function-wide guarantee, so the pinned value holds
/// wherever the register could be read rather than at one site.
fn right_bits(function: &SelectedFunction, side: CompareRight) -> Option<u64> {
    match side {
        CompareRight::Register(register) => materialized_bits(function, register).ok(),
        CompareRight::Literal(bits) => Some(bits),
    }
}

/// Whether `shadow` computes the same subtraction `victim` does, side by
/// side in the compare's own direction — the validator's own equivalence
/// audit. A side coincides by register identity — the same virtual
/// register on both, whose value could still drift between shadow and
/// compare, so it lands in `audited` for the caller's path audit — or by
/// literal: each side resolves to the same bit pattern, a register
/// through its unique `MaterializeI64` producer and an encoded literal
/// through its own bits. A side that cannot be proven equal — unshared
/// registers without matching materializations, or a register against an
/// unmatched literal — fails, and the failure is what keeps a swapped
/// operand pair a different publication.
fn sides_equal(
    function: &SelectedFunction,
    victim: (VirtualRegisterId, CompareRight),
    shadow: (VirtualRegisterId, CompareRight),
    audited: &mut BTreeSet<VirtualRegisterId>,
) -> bool {
    let left_equal = if victim.0 == shadow.0 {
        audited.insert(victim.0);
        true
    } else {
        matches!(
            (
                materialized_bits(function, victim.0).ok(),
                materialized_bits(function, shadow.0).ok()
            ),
            (Some(victim_bits), Some(shadow_bits)) if victim_bits == shadow_bits
        )
    };
    if !left_equal {
        return false;
    }
    match (victim.1, shadow.1) {
        (CompareRight::Register(victim_register), CompareRight::Register(shadow_register))
            if victim_register == shadow_register =>
        {
            audited.insert(victim_register);
            true
        }
        (victim_right, shadow_right) => matches!(
            (
                right_bits(function, victim_right),
                right_bits(function, shadow_right)
            ),
            (Some(victim_bits), Some(shadow_bits)) if victim_bits == shadow_bits
        ),
    }
}

/// The validator's own backward operand audit: every operand register the
/// compare and one of its shadow sites read in common must keep its value
/// between the shadow's execution and the arrival at the compare. The
/// exposed positions are exactly those that can reach the compare
/// backward without crossing a site in that unit's reaching set — a
/// shadow's execution resets the unit's binding segment because it
/// republishes the identical flag value, so writes before it are rescued,
/// while another unit's shadow republishes only its own unit and cannot
/// stand in. The walk marks each visited `(block, position)` once: body
/// positions expand to the position before them, a block's first position
/// expands to its predecessor blocks' terminator positions, and each
/// crossed edge's register transports face the same audit — an edge
/// parameter the edge defines for its target is a write of the target's
/// register.
fn audit_operand_stability(
    function: &SelectedFunction,
    predecessors: &[Vec<usize>],
    block_index: usize,
    compare_index: usize,
    unit_sites: &BTreeSet<EventSite>,
    registers: &BTreeSet<VirtualRegisterId>,
) -> Result<(), EquivalentCompareError> {
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
            return Err(EquivalentCompareError::UnsupportedUse);
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
                        return Err(EquivalentCompareError::UnsupportedUse);
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
                            return Err(EquivalentCompareError::UnsupportedUse);
                        }
                    }
                }
            }
            frontier.push((predecessor, function.blocks[predecessor].instructions.len()));
        }
    }
    Ok(())
}

/// Reconstruct the legality of removing `compare` from first principles:
/// locate the instruction, verify the canonical flag-publisher shape
/// against the bound constraint row and the roster, refuse any call or
/// memory-access row that names it, resolve every published unit's
/// reaching events through the shared condition-state walk, and re-decide
/// each resolved site's equivalence and the shared operands' stability on
/// the validator's own audits. Nothing in this audit reads the producer's
/// admission decision.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
) -> Result<Reconstructed<'source>, EquivalentCompareError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(EquivalentCompareError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(EquivalentCompareError::SourceMismatch)?;
    let (block_index, compare_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == compare)
                .map(|compare_index| (block_index, compare_index))
        })
        .ok_or(EquivalentCompareError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let compare_instruction = &block.instructions[compare_index];
    // The removed instruction must be one of the three compare forms in
    // the emitted flag-publisher shape — every operand a plain `Use` at
    // its canonical position, no implicit uses the removal would silently
    // drop, and a nonempty published surface — with an immediate payload
    // the form actually encodes.
    let victim_sides =
        compare_sides(compare_instruction).ok_or(EquivalentCompareError::UnsupportedInstruction)?;
    // The compare's own constraint row must declare the same all-`use`
    // operand shape at the classes the roster assigns, and the same
    // implicit surface the instruction carries: a row that disagrees
    // would change what removing the instruction means.
    let arity = compare_instruction.operands.len();
    let row = environment
        .constraint(compare_instruction.constraint)
        .ok_or(EquivalentCompareError::ConstraintMismatch)?;
    if row.operands.len() != arity
        || row.implicit_uses != compare_instruction.implicit_uses
        || row.implicit_defs != compare_instruction.implicit_defs
        || row.clobbers != compare_instruction.clobbers
    {
        return Err(EquivalentCompareError::ConstraintMismatch);
    }
    for (position, operand) in compare_instruction.operands.iter().enumerate() {
        let register = function
            .virtual_registers
            .iter()
            .find(|register| register.id == operand.virtual_register)
            .ok_or(EquivalentCompareError::ConstraintMismatch)?;
        if row.operands[position].operand != operand.operand
            || row.operands[position].access != RegisterOperandAccess::Use
            || row.operands[position].class != register.class
        {
            return Err(EquivalentCompareError::ConstraintMismatch);
        }
    }
    // Removing the compare orphans anything that names it: call contracts
    // and memory-access rows bind instructions by identity, and neither
    // may claim the compare.
    if function
        .calls
        .iter()
        .any(|call| call.instruction == compare)
        || function
            .memory_accesses
            .iter()
            .any(|access| access.instruction == compare)
    {
        return Err(EquivalentCompareError::UnsupportedInstruction);
    }
    // Every unit the compare publishes — defined or clobbered — must
    // reach the compare carrying the state the compare republishes. The
    // shared walk resolves each unit's reaching set: the last in-block
    // event when one precedes the compare, otherwise every site a path
    // may last have observed across edges. A defined unit needs every
    // site to be a canonical compare that defines the unit and computes
    // the same subtraction — form identity is not required, only the
    // coinciding operand values — and the unit's site set then bounds the
    // operand audit: no position exposed between a unit site and the
    // compare may write a register the two compares share by identity. A
    // clobbered unit needs every site to leave it unspecified: a clobber,
    // never a definition whose flags the removal would expose.
    let (successors, predecessors) = adjacency(function);
    let entry = entry_index(function).ok_or(EquivalentCompareError::SourceMismatch)?;
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
        .map_err(|_| EquivalentCompareError::UnsupportedUse)?;
        let mut unit_sites = BTreeSet::new();
        let mut audited = BTreeSet::new();
        for site in sites {
            let event = instruction_at(function, site);
            if defined {
                let site_sides =
                    compare_sides(event).ok_or(EquivalentCompareError::UnsupportedUse)?;
                if !event.implicit_defs.contains(&unit)
                    || event.clobbers.contains(&unit)
                    || !sides_equal(function, victim_sides, site_sides, &mut audited)
                {
                    return Err(EquivalentCompareError::UnsupportedUse);
                }
                unit_sites.insert(site);
            } else if !event.clobbers.contains(&unit) || event.implicit_defs.contains(&unit) {
                return Err(EquivalentCompareError::UnsupportedUse);
            }
        }
        if defined {
            audit_operand_stability(
                function,
                &predecessors,
                block_index,
                compare_index,
                &unit_sites,
                &audited,
            )?;
        }
    }
    Ok(Reconstructed {
        function,
        function_index,
        block_index,
        block: block.id,
        compare_index,
        successors,
        units: units.len(),
    })
}

/// The validation work this audit performs, in the measured-step contract
/// the family publishes: one step per block plus one per instruction
/// across the plan, a second scan of the reconstructed function together
/// with its roster, call, and access rows, then the reaching walk's setup
/// — every edge's target resolution, the predecessor fill, and the cone
/// mark — plus per published unit the in-block prefix scan, the per-block
/// last-event table, the bounded fixpoint propagation, and the per-site
/// equivalence checks' unique-producer scans, and finally the backward
/// operand audit once per unit over stream positions and crossed edge
/// surfaces.
fn measured_steps(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
    successors: &[Vec<usize>],
    units: usize,
) -> Result<u64, EquivalentCompareError> {
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    let block_count = function.blocks.len();
    let edge_count = successors
        .iter()
        .try_fold(0usize, |total, targets| total.checked_add(targets.len()))
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    let elements = function_scan
        .checked_add(1)
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    let pops = block_count
        .checked_mul(
            elements
                .checked_add(1)
                .ok_or(EquivalentCompareError::IdentityOverflow)?,
        )
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    let widest_out = successors
        .iter()
        .map(|targets| targets.len())
        .max()
        .unwrap_or(0);
    let per_unit = function_scan
        .checked_add(block_count)
        .and_then(|total| total.checked_add(pops))
        .and_then(|total| total.checked_add(pops.checked_mul(widest_out)?.checked_mul(elements)?))
        .and_then(|total| total.checked_add(elements.checked_mul(function_scan)?.checked_mul(4)?))
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    let walk_setup = edge_count
        .checked_mul(
            block_count
                .checked_add(1)
                .ok_or(EquivalentCompareError::IdentityOverflow)?,
        )
        .and_then(|total| total.checked_add(block_count))
        .and_then(|total| total.checked_add(edge_count))
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
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
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    let audit = units
        .checked_mul(
            function_scan
                .checked_add(edge_count)
                .and_then(|total| total.checked_add(transport_rows))
                .ok_or(EquivalentCompareError::IdentityOverflow)?,
        )
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
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
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    u64::try_from(steps).map_err(|_| EquivalentCompareError::IdentityOverflow)
}

/// Build the function the contract demands from the validator's own
/// record: the compare leaves its block and boundary settlements shift
/// over the removed ordinal. The producer's `apply` is not consulted;
/// both sides derive the same function from the source alone.
fn expect(reconstructed: &Reconstructed<'_>) -> Result<SelectedFunction, EquivalentCompareError> {
    let mut expected = reconstructed.function.clone();
    let block = expected
        .blocks
        .get_mut(reconstructed.block_index)
        .ok_or(EquivalentCompareError::SourceMismatch)?;
    if block.id != reconstructed.block {
        return Err(EquivalentCompareError::SourceMismatch);
    }
    // Positions at or before the removed ordinal name instructions that
    // stay put; every later position — including the after-body position —
    // shifts one ordinal earlier. Settlements name hosted boundary
    // operations, never flag units or registers, so the removal itself is
    // invisible to them.
    let body = block.instructions.len();
    expected.boundary_settlements = expected
        .boundary_settlements
        .into_iter()
        .map(|mut settlement| {
            if settlement.block == reconstructed.block {
                let position = settlement.instruction_index as usize;
                if position > body {
                    return Err(EquivalentCompareError::SourceMismatch);
                }
                if position > reconstructed.compare_index {
                    settlement.instruction_index = u32::try_from(position - 1)
                        .map_err(|_| EquivalentCompareError::IdentityOverflow)?;
                }
            }
            Ok(settlement)
        })
        .collect::<Result<Vec<_>, _>>()?;
    block.instructions.remove(reconstructed.compare_index);
    Ok(expected)
}

/// Reinsert the compare and the source settlements: undoing the
/// validator's expected edit must restore the complete source by content —
/// every other instruction, register, call, and function included.
fn restore(
    reconstructed: &Reconstructed<'_>,
    proposed: &SelectedInstructionPlan,
) -> Result<SelectedInstructionPlan, EquivalentCompareError> {
    let mut restored = proposed.clone();
    let function = restored
        .functions
        .get_mut(reconstructed.function_index)
        .ok_or(EquivalentCompareError::ReplayMismatch)?;
    let block = function
        .blocks
        .get_mut(reconstructed.block_index)
        .ok_or(EquivalentCompareError::ReplayMismatch)?;
    if block.id != reconstructed.block || block.instructions.len() < reconstructed.compare_index {
        return Err(EquivalentCompareError::ReplayMismatch);
    }
    block.instructions.insert(
        reconstructed.compare_index,
        reconstructed.function.blocks[reconstructed.block_index].instructions
            [reconstructed.compare_index]
            .clone(),
    );
    function.boundary_settlements = reconstructed.function.boundary_settlements.clone();
    Ok(restored)
}

/// Independently consume the proposed program: the validator reconstructs
/// the removal's preconditions from the source, requires the proposal to
/// equal the function its own record produces, and restores the complete
/// source by content — every other instruction, register, call, and
/// settlement included. The producer's `admission::admit` is never
/// consulted, so a wrong legality decision fails here even when the
/// proposal matches the edit the producer emitted.
pub fn validate_equivalent_compare(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedEquivalentCompare, EquivalentCompareError> {
    let reconstructed = reconstruct(source, function_index, compare, environment)?;
    if measured_steps(
        source.selected_plan(),
        reconstructed.function,
        &reconstructed.successors,
        reconstructed.units,
    )? > budget.validation_steps()
    {
        return Err(EquivalentCompareError::WorkBudgetExceeded);
    }
    if proposed.functions.get(function_index) != Some(&expect(&reconstructed)?) {
        return Err(EquivalentCompareError::ReplayMismatch);
    }
    if restore(&reconstructed, &proposed)? != *source.selected_plan() {
        return Err(EquivalentCompareError::ReplayMismatch);
    }
    Ok(ValidatedEquivalentCompare {
        receipt: EquivalentCompareReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}

#[cfg(test)]
mod independence_tests {
    use std::sync::Arc;

    use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
    use optimization_unit::ValueDefinitionSite;
    use register_environment::{
        ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
    };
    use register_model::RegisterInstructionConstraint;
    use selected_instructions::{
        SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
        SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedOperand,
        SelectedTerminator, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{
        BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
        ScalarType, ValueId,
    };
    use target::NativeTarget;
    use target_operations_to_selected_instructions::selected_instruction_plan_identity;
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::{
        EquivalentCompareError, EquivalentCompareReceipt, ValidatedEquivalentCompare,
        validate_equivalent_compare,
    };

    const LOAD: SelectedInstructionId = SelectedInstructionId(2);
    const SECOND_LOAD: SelectedInstructionId = SelectedInstructionId(3);
    const MATERIALIZE: SelectedInstructionId = SelectedInstructionId(4);
    const SHADOW: SelectedInstructionId = SelectedInstructionId(5);
    const EQUIVALENT: SelectedInstructionId = SelectedInstructionId(6);
    const TERMINAL: SelectedInstructionId = SelectedInstructionId(7);
    const MOVED: SelectedInstructionId = SelectedInstructionId(9);

    const POINTER: VirtualRegisterId = VirtualRegisterId(0);
    const INPUT: VirtualRegisterId = VirtualRegisterId(1);
    const LITERAL: VirtualRegisterId = VirtualRegisterId(2);
    const OTHER: VirtualRegisterId = VirtualRegisterId(3);

    fn budget() -> OptimizationWorkBudget {
        OptimizationWorkBudget::new(100, 100, 100_000, 100, 100).unwrap()
    }

    fn instruction(
        id: SelectedInstructionId,
        kind: SelectedInstructionKind,
        row: &RegisterInstructionConstraint,
        registers: &[VirtualRegisterId],
    ) -> SelectedInstruction {
        SelectedInstruction {
            id,
            kind,
            constraint: row.key,
            operands: row
                .operands
                .iter()
                .zip(registers)
                .map(|(operand, register)| SelectedOperand {
                    operand: operand.operand,
                    virtual_register: *register,
                    access: operand.access,
                    class: operand.class,
                    fixed_view: operand.fixed_view,
                    tied_to: operand.tied_to,
                    early_clobber: operand.early_clobber,
                })
                .collect(),
            implicit_uses: row.implicit_uses.clone(),
            implicit_defs: row.implicit_defs.clone(),
            clobbers: row.clobbers.clone(),
            provenance: Default::default(),
        }
    }

    fn u64_scalar() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
    }

    /// `r1 = load8 r0; r3 = load8 r0; r2 = materialize 7; compare_imm r1, 7;
    /// compare r1, r2; return` — the fixture's removal is legal: the
    /// immediate-form shadow computes `r1 - 7` and `r2`'s unique
    /// materialization pins the victim's right operand to the same literal,
    /// so the register-form victim republishes flag state every path
    /// already observes. `edit` then breaks one precondition, and the
    /// returned proposal is what a defective producer would emit anyway:
    /// the compare dropped though the legality fails.
    fn forged(
        target: NativeTarget,
        edit: impl FnOnce(&mut SelectedFunction, &ValidatedTargetRegisterEnvironment),
    ) -> (
        ValidatedEquivalentCompare,
        ValidatedTargetRegisterEnvironment,
        SelectedInstructionPlan,
    ) {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.selected_keys();
        let load_row = environment.constraint(keys.load8.unwrap()).unwrap();
        let materialize_row = environment.constraint(keys.materialize_i64).unwrap();
        let compare_row = environment.constraint(keys.compare_i64).unwrap();
        let immediate_row = environment.constraint(keys.compare_i64_immediate).unwrap();
        let return_row = environment.constraint(keys.return_unit).unwrap();
        let class = compare_row.operands[0].class;
        let machine = MachineId::new(1).unwrap();
        let function = SelectedFunction {
            machine,
            attachment: None,
            provenance: Default::default(),
            structural: None,
            local_storage_slots: Vec::new(),
            outgoing_arguments: Vec::new(),
            calls: Vec::new(),
            normalized_foreign_calls: Vec::new(),
            memory_accesses: Vec::new(),
            boundary_settlements: Vec::new(),
            entry_block: SelectedBlockId(0),
            virtual_registers: vec![
                VirtualRegister {
                    id: POINTER,
                    scalar_type: u64_scalar(),
                    class,
                    origin: VirtualRegisterOrigin::EntryParameter {
                        source_value: ValueId::new(1).unwrap(),
                        parameter_index: 0,
                    },
                    definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
                    entry_fixed_view: None,
                },
                VirtualRegister {
                    id: INPUT,
                    scalar_type: u64_scalar(),
                    class,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: LOAD,
                        source_value: ValueId::new(2).unwrap(),
                    },
                    definition_site: None,
                    entry_fixed_view: None,
                },
                VirtualRegister {
                    id: LITERAL,
                    scalar_type: u64_scalar(),
                    class,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: MATERIALIZE,
                        source_value: ValueId::new(4).unwrap(),
                    },
                    definition_site: None,
                    entry_fixed_view: None,
                },
                VirtualRegister {
                    id: OTHER,
                    scalar_type: u64_scalar(),
                    class,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: SECOND_LOAD,
                        source_value: ValueId::new(3).unwrap(),
                    },
                    definition_site: None,
                    entry_fixed_view: None,
                },
            ],
            blocks: vec![SelectedBlock {
                id: SelectedBlockId(0),
                origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                instructions: vec![
                    instruction(
                        LOAD,
                        SelectedInstructionKind::Load8 { byte_offset: 0 },
                        load_row,
                        &[POINTER, INPUT],
                    ),
                    instruction(
                        SECOND_LOAD,
                        SelectedInstructionKind::Load8 { byte_offset: 0 },
                        load_row,
                        &[POINTER, OTHER],
                    ),
                    instruction(
                        MATERIALIZE,
                        SelectedInstructionKind::MaterializeI64 {
                            value: IntegerValue::Unsigned(7),
                        },
                        materialize_row,
                        &[LITERAL],
                    ),
                    instruction(
                        SHADOW,
                        SelectedInstructionKind::CompareI64Immediate {
                            immediate: IntegerValue::Unsigned(7),
                        },
                        immediate_row,
                        &[INPUT],
                    ),
                    instruction(
                        EQUIVALENT,
                        SelectedInstructionKind::CompareI64,
                        compare_row,
                        &[INPUT, LITERAL],
                    ),
                ],
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        TERMINAL,
                        SelectedInstructionKind::ReturnUnit,
                        return_row,
                        &[],
                    ),
                    psi_return_edge: EdgeId::new(1).unwrap(),
                },
            }],
        };
        let plan = SelectedInstructionPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            target,
            entry: machine,
            functions: vec![function].into(),
        };
        let identity = selected_instruction_plan_identity(&plan);
        let mut source = ValidatedEquivalentCompare {
            receipt: EquivalentCompareReceipt {
                source_selected: identity,
                transformed_selected: identity,
                optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
                fuel_schedule: plan.fuel_schedule,
            },
            transformed: Arc::new(plan),
        };
        edit(
            &mut Arc::make_mut(&mut source.transformed).functions[0],
            &environment,
        );
        let identity = selected_instruction_plan_identity(&source.transformed);
        source.receipt.source_selected = identity;
        source.receipt.transformed_selected = identity;
        // The proposal a defective producer would emit: the compare
        // removed though the legality preconditions no longer hold.
        let mut proposed = source.transformed().clone();
        proposed.functions[0].blocks[0]
            .instructions
            .retain(|instruction| instruction.id != EQUIVALENT);
        (source, environment, proposed)
    }

    /// The shadow encodes 8 where the victim's right operand materializes
    /// 7 — a different subtraction, so the removal is a legality error. A
    /// producer that admitted it anyway would publish a plan whose compare
    /// is gone while the reaching shadow publishes different flags; the
    /// validator's own equivalence audit must refuse the legality with
    /// `UnsupportedUse`, not merely diff the proposal. Feeding that forged
    /// proposal is the observable proof that validation no longer relies
    /// on the producer's admission routine.
    #[test]
    fn validator_refuses_a_proposal_past_a_divergent_shadow() {
        let target = NativeTarget::linux_x64();
        let (source, environment, proposed) = forged(target, |function, environment| {
            let immediate_row = environment
                .constraint(environment.selected_keys().compare_i64_immediate)
                .unwrap()
                .clone();
            function.blocks[0].instructions[3] = instruction(
                SHADOW,
                SelectedInstructionKind::CompareI64Immediate {
                    immediate: IntegerValue::Unsigned(8),
                },
                &immediate_row,
                &[INPUT],
            );
        });
        assert_eq!(
            validate_equivalent_compare(&source, 0, EQUIVALENT, &environment, budget(), proposed)
                .unwrap_err(),
            EquivalentCompareError::UnsupportedUse
        );
    }

    /// A write to the shared left register between the shadow and the
    /// victim leaves the subtraction's inputs unproven: the victim may
    /// compute flags from a different `r1` than the shadow observed. The
    /// validator's own backward stability walk must refuse the forged
    /// proposal.
    #[test]
    fn validator_refuses_a_proposal_past_operand_drift() {
        let target = NativeTarget::linux_x64();
        let (source, environment, proposed) = forged(target, |function, environment| {
            let copy_row = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap()
                .clone();
            function.blocks[0].instructions.insert(
                4,
                instruction(
                    MOVED,
                    SelectedInstructionKind::CopyI64,
                    &copy_row,
                    &[OTHER, INPUT],
                ),
            );
        });
        assert_eq!(
            validate_equivalent_compare(&source, 0, EQUIVALENT, &environment, budget(), proposed)
                .unwrap_err(),
            EquivalentCompareError::UnsupportedUse
        );
    }

    /// A shadow that clobbers the unit rather than defining it leaves the
    /// victim's publication unmatched: removing the victim would expose an
    /// unspecified unit where it published real flags. The validator's own
    /// role audit must refuse the forged proposal.
    #[test]
    fn validator_refuses_a_proposal_past_a_clobbering_shadow() {
        let target = NativeTarget::linux_x64();
        let (source, environment, proposed) = forged(target, |function, _| {
            let shadow = &mut function.blocks[0].instructions[3];
            shadow.clobbers = shadow.implicit_defs.clone();
            shadow.implicit_defs = Vec::new();
        });
        assert_eq!(
            validate_equivalent_compare(&source, 0, EQUIVALENT, &environment, budget(), proposed)
                .unwrap_err(),
            EquivalentCompareError::UnsupportedUse
        );
    }

    /// With no earlier flag event there is no shadow at all: the entry's
    /// condition state is unknown, so the reaching walk resolves nothing a
    /// removal could republish. The validator's own walk must refuse the
    /// forged proposal.
    #[test]
    fn validator_refuses_a_proposal_past_an_unknown_entry_state() {
        let target = NativeTarget::linux_x64();
        let (source, environment, proposed) = forged(target, |function, _| {
            function.blocks[0].instructions.remove(3);
        });
        assert_eq!(
            validate_equivalent_compare(&source, 0, EQUIVALENT, &environment, budget(), proposed)
                .unwrap_err(),
            EquivalentCompareError::UnsupportedUse
        );
    }
}

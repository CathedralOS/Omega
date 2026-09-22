//! Independent validation of redundant-compare removal.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! removal's legality from the source records — the named instruction's
//! compare kind and canonical flag-publisher shape against the bound
//! constraint row, the call and memory-access rosters that must not name
//! it, the flag-equivalent shadows every published unit's reaching events
//! must resolve to, and the backward walk proving no position exposed
//! between a unit's shadow sites and the compare writes a shared operand
//! register — then rebuilds the function the contract demands and
//! requires the proposal to equal it. Restoring the removed compare and
//! the source settlements must reproduce the complete source by content.
//! A producer admission error therefore fails validation even when the
//! proposal is exactly what that producer emitted.
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

use super::{RedundantCompareError, RedundantCompareReceipt, ValidatedRedundantCompare};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{edge_surface, terminator_successors};
use crate::rewrites::unexecuted::condition_state::{
    EventSite, adjacency, backward_cone, entry_index, instruction_at, reaching_events,
};

/// The validator's own reconstruction of the removal the contract permits:
/// the admitted compare's coordinates, its block's identity, the
/// block-indexed adjacency the walks ran over, and the count of published
/// units for the measured-step contract. It shares no state with the
/// producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    function_index: usize,
    block_index: usize,
    block: SelectedBlockId,
    compare_index: usize,
    /// The resolved-index successor lists `adjacency` built: edges naming
    /// blocks the function does not contain participate in neither walk
    /// nor the edge count the contract measures.
    successors: Vec<Vec<usize>>,
    /// The distinct condition-state units the compare publishes, collected
    /// by the validator's own audit for the measured-step contract.
    units: usize,
}

/// Whether `kind` is one of the three flag publishers whose whole effect is
/// the condition-state surface it defines, with the operand arity the
/// emitted form carries. The validator re-decides the compare grammar from
/// the instruction kind alone.
fn compare_arity(kind: SelectedInstructionKind) -> Option<usize> {
    match kind {
        SelectedInstructionKind::CompareI64 => Some(2),
        SelectedInstructionKind::CompareI64Immediate { .. }
        | SelectedInstructionKind::CompareI64Zero => Some(1),
        _ => None,
    }
}

/// The plain-`Use` operand registers of a canonical flag publisher, or
/// none: the validator re-decides the emitted shape — canonical operand
/// positions, no implicit uses, and a nonempty published surface — so
/// pairwise register equality really is value equality, for the victim and
/// every resolved shadow alike.
fn canonical_compare_registers(
    instruction: &SelectedInstruction,
) -> Option<Vec<VirtualRegisterId>> {
    if compare_arity(instruction.kind)? != instruction.operands.len()
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

/// Every operand register the compare and one of its shadow sites read in
/// common must keep its value between the shadow's execution and the
/// arrival at the compare. The validator's own backward walk computes the
/// exposed positions: those that can reach the compare without crossing a
/// site in that unit's reaching set — a shadow's execution resets the
/// unit's binding segment because it republishes the identical flag value,
/// so writes before it are rescued, while another unit's shadow republishes
/// only its own unit and cannot stand in. The walk marks each visited
/// `(block, position)` once: body positions expand to the position before
/// them, a block's first position expands to its predecessor blocks'
/// terminator positions, and each crossed edge's register transports face
/// the same audit — an edge parameter the edge defines for its target is a
/// write of the target's register.
fn audit_stable_paths(
    function: &SelectedFunction,
    predecessors: &[Vec<usize>],
    block_index: usize,
    compare_index: usize,
    unit_sites: &BTreeSet<EventSite>,
    registers: &BTreeSet<VirtualRegisterId>,
) -> Result<(), RedundantCompareError> {
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
            return Err(RedundantCompareError::UnsupportedUse);
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
                        return Err(RedundantCompareError::UnsupportedUse);
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
                            return Err(RedundantCompareError::UnsupportedUse);
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
/// memory-access row that names it, then resolve every published unit's
/// reaching events on the module's shared condition-state walk and audit
/// the operand interval the validator's own backward walk exposes. A unit
/// the compare defines needs every site to be a flag-equivalent compare —
/// identical kind, identical operand registers — that defines rather than
/// clobbers it; a unit the compare only clobbers needs every site to leave
/// it unspecified. Nothing in this audit reads the producer's admission
/// decision.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
) -> Result<Reconstructed<'source>, RedundantCompareError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(RedundantCompareError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(RedundantCompareError::SourceMismatch)?;
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
        .ok_or(RedundantCompareError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let compare_instruction = &block.instructions[compare_index];
    // The removed instruction must be one of the three compare forms in
    // the emitted flag-publisher shape: every operand a plain `Use` at its
    // canonical position, no implicit uses the removal would silently drop,
    // and a nonempty published surface.
    let compare_registers = canonical_compare_registers(compare_instruction)
        .ok_or(RedundantCompareError::UnsupportedInstruction)?;
    let operand_registers: BTreeSet<VirtualRegisterId> =
        compare_registers.iter().copied().collect();
    // The compare's own constraint row must declare the same all-`use`
    // operand shape at the classes the roster assigns, and the same
    // implicit surface the instruction carries: a row that disagrees would
    // change what removing the instruction means.
    let arity = compare_instruction.operands.len();
    let row = environment
        .constraint(compare_instruction.constraint)
        .ok_or(RedundantCompareError::ConstraintMismatch)?;
    if row.operands.len() != arity
        || row.implicit_uses != compare_instruction.implicit_uses
        || row.implicit_defs != compare_instruction.implicit_defs
        || row.clobbers != compare_instruction.clobbers
    {
        return Err(RedundantCompareError::ConstraintMismatch);
    }
    for (position, operand) in compare_instruction.operands.iter().enumerate() {
        let register = function
            .virtual_registers
            .iter()
            .find(|register| register.id == operand.virtual_register)
            .ok_or(RedundantCompareError::ConstraintMismatch)?;
        if row.operands[position].operand != operand.operand
            || row.operands[position].access != RegisterOperandAccess::Use
            || row.operands[position].class != register.class
        {
            return Err(RedundantCompareError::ConstraintMismatch);
        }
    }
    // Removing the compare orphans anything that names it: call contracts
    // and memory-access rows bind instructions by identity, and neither may
    // claim the compare.
    if function
        .calls
        .iter()
        .any(|call| call.instruction == compare)
        || function
            .memory_accesses
            .iter()
            .any(|access| access.instruction == compare)
    {
        return Err(RedundantCompareError::UnsupportedInstruction);
    }
    // The flag walk crosses block boundaries: build the block-indexed
    // adjacency once, locate the entry block whose unseeded condition state
    // bounds every path, and mark the cone of blocks that can reach the
    // compare's block — only their entry sets can feed the reaching events.
    let (successors, predecessors) = adjacency(function);
    let entry = entry_index(function).ok_or(RedundantCompareError::SourceMismatch)?;
    let cone = backward_cone(&predecessors, block_index);
    // Every unit the compare publishes — defined or clobbered — must reach
    // the compare carrying the state the compare republishes. A defined
    // unit needs every site to be a flag-equivalent compare that defines
    // it, and its site set then bounds the operand audit; a clobbered unit
    // needs every site to leave it unspecified — a clobber, never a
    // definition whose flags the removal would expose. The validator
    // collects and audits the same sets on its own, so a producer that
    // admitted a divergent shadow or an exposed operand write cannot pass
    // validation.
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
        .map_err(|_| RedundantCompareError::UnsupportedUse)?;
        let mut unit_sites = BTreeSet::new();
        for site in sites {
            let event = instruction_at(function, site);
            if defined {
                let site_registers = canonical_compare_registers(event)
                    .ok_or(RedundantCompareError::UnsupportedUse)?;
                if event.kind != compare_instruction.kind
                    || site_registers != compare_registers
                    || !event.implicit_defs.contains(&unit)
                    || event.clobbers.contains(&unit)
                {
                    return Err(RedundantCompareError::UnsupportedUse);
                }
                unit_sites.insert(site);
            } else if !event.clobbers.contains(&unit) || event.implicit_defs.contains(&unit) {
                return Err(RedundantCompareError::UnsupportedUse);
            }
        }
        if defined {
            audit_stable_paths(
                function,
                &predecessors,
                block_index,
                compare_index,
                &unit_sites,
                &operand_registers,
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
/// across the plan, a second scan of the reconstructed function's blocks
/// together with its roster, call, and access rows, the shared walk's
/// setup and fixpoint bound, and the backward operand audit at one stream
/// scan plus the crossed edges' transport surface per published unit.
fn measured_steps(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
    successors: &[Vec<usize>],
    units: usize,
) -> Result<u64, RedundantCompareError> {
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(RedundantCompareError::IdentityOverflow)?;
    let block_count = function.blocks.len();
    let edge_count = successors
        .iter()
        .try_fold(0usize, |total, targets| total.checked_add(targets.len()))
        .ok_or(RedundantCompareError::IdentityOverflow)?;
    let elements = function_scan
        .checked_add(1)
        .ok_or(RedundantCompareError::IdentityOverflow)?;
    let pops = block_count
        .checked_mul(
            elements
                .checked_add(1)
                .ok_or(RedundantCompareError::IdentityOverflow)?,
        )
        .ok_or(RedundantCompareError::IdentityOverflow)?;
    let widest_out = successors
        .iter()
        .map(|targets| targets.len())
        .max()
        .unwrap_or(0);
    let per_unit = function_scan
        .checked_add(block_count)
        .and_then(|total| total.checked_add(pops))
        .and_then(|total| total.checked_add(pops.checked_mul(widest_out)?.checked_mul(elements)?))
        .ok_or(RedundantCompareError::IdentityOverflow)?;
    let walk_setup = edge_count
        .checked_mul(
            block_count
                .checked_add(1)
                .ok_or(RedundantCompareError::IdentityOverflow)?,
        )
        .and_then(|total| total.checked_add(block_count))
        .and_then(|total| total.checked_add(edge_count))
        .ok_or(RedundantCompareError::IdentityOverflow)?;
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
        .ok_or(RedundantCompareError::IdentityOverflow)?;
    let audit = units
        .checked_mul(
            function_scan
                .checked_add(edge_count)
                .and_then(|total| total.checked_add(transport_rows))
                .ok_or(RedundantCompareError::IdentityOverflow)?,
        )
        .ok_or(RedundantCompareError::IdentityOverflow)?;
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
        .ok_or(RedundantCompareError::IdentityOverflow)?;
    u64::try_from(steps).map_err(|_| RedundantCompareError::IdentityOverflow)
}

/// Build the function the contract demands from the validator's own
/// record: the compare leaves its block and boundary settlements shift
/// over the removed ordinal. The producer's `apply` is not consulted;
/// both sides derive the same function from the source alone.
fn expect(reconstructed: &Reconstructed<'_>) -> Result<SelectedFunction, RedundantCompareError> {
    let mut expected = reconstructed.function.clone();
    let block = expected
        .blocks
        .get_mut(reconstructed.block_index)
        .ok_or(RedundantCompareError::SourceMismatch)?;
    if block.id != reconstructed.block {
        return Err(RedundantCompareError::SourceMismatch);
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
                    return Err(RedundantCompareError::SourceMismatch);
                }
                if position > reconstructed.compare_index {
                    settlement.instruction_index = u32::try_from(position - 1)
                        .map_err(|_| RedundantCompareError::IdentityOverflow)?;
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
) -> Result<SelectedInstructionPlan, RedundantCompareError> {
    let mut restored = proposed.clone();
    let function = restored
        .functions
        .get_mut(reconstructed.function_index)
        .ok_or(RedundantCompareError::ReplayMismatch)?;
    let block = function
        .blocks
        .get_mut(reconstructed.block_index)
        .ok_or(RedundantCompareError::ReplayMismatch)?;
    if block.id != reconstructed.block || block.instructions.len() < reconstructed.compare_index {
        return Err(RedundantCompareError::ReplayMismatch);
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
pub fn validate_redundant_compare(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedRedundantCompare, RedundantCompareError> {
    let reconstructed = reconstruct(source, function_index, compare, environment)?;
    if measured_steps(
        source.selected_plan(),
        reconstructed.function,
        &reconstructed.successors,
        reconstructed.units,
    )? > budget.validation_steps()
    {
        return Err(RedundantCompareError::WorkBudgetExceeded);
    }
    if proposed.functions.get(function_index) != Some(&expect(&reconstructed)?) {
        return Err(RedundantCompareError::ReplayMismatch);
    }
    if restore(&reconstructed, &proposed)? != *source.selected_plan() {
        return Err(RedundantCompareError::ReplayMismatch);
    }
    Ok(ValidatedRedundantCompare {
        receipt: RedundantCompareReceipt {
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
    use register_environment::baseline_target_register_environment;
    use register_model::RegisterInstructionConstraint;
    use selected_instructions::{
        SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
        SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedOperand,
        SelectedTerminator, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{
        BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, ScalarType,
        ValueId,
    };
    use target::NativeTarget;
    use target_operations_to_selected_instructions::selected_instruction_plan_identity;
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::{
        RedundantCompareError, RedundantCompareReceipt, ValidatedRedundantCompare,
        validate_redundant_compare,
    };

    const LOAD: SelectedInstructionId = SelectedInstructionId(2);
    const SHADOW: SelectedInstructionId = SelectedInstructionId(3);
    const REWRITE: SelectedInstructionId = SelectedInstructionId(4);
    const COMPARE: SelectedInstructionId = SelectedInstructionId(5);
    const TERMINAL: SelectedInstructionId = SelectedInstructionId(7);

    const POINTER: VirtualRegisterId = VirtualRegisterId(0);
    const INPUT: VirtualRegisterId = VirtualRegisterId(1);

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

    /// `r1 = load8 r0; compare r1, r1; r1 = load8 r0; compare r1, r1;
    /// return` — the second load rewrites the shared operand register
    /// between the shadow and the victim, so the same register no longer
    /// names the same value and the removal is a legality error. A
    /// producer that admitted it anyway would publish a plan whose victim
    /// is gone while its republication was never proven; the validator's
    /// own backward operand audit must refuse with `UnsupportedUse`, not
    /// merely diff the proposal. Feeding that forged proposal is the
    /// observable proof that validation no longer relies on the producer's
    /// admission routine.
    #[test]
    fn validator_refuses_a_proposal_its_own_audit_rejects() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.selected_keys();
        let load_row = environment.constraint(keys.load8.unwrap()).unwrap();
        let compare_row = environment.constraint(keys.compare_i64).unwrap();
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
                        SHADOW,
                        SelectedInstructionKind::CompareI64,
                        compare_row,
                        &[INPUT, INPUT],
                    ),
                    instruction(
                        REWRITE,
                        SelectedInstructionKind::Load8 { byte_offset: 0 },
                        load_row,
                        &[POINTER, INPUT],
                    ),
                    instruction(
                        COMPARE,
                        SelectedInstructionKind::CompareI64,
                        compare_row,
                        &[INPUT, INPUT],
                    ),
                ],
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        TERMINAL,
                        SelectedInstructionKind::ReturnUnit,
                        return_row,
                        &[],
                    ),
                    psi_return_edge: EdgeId::new(2).unwrap(),
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
        let source = ValidatedRedundantCompare {
            receipt: RedundantCompareReceipt {
                source_selected: identity,
                transformed_selected: identity,
                optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
                fuel_schedule: plan.fuel_schedule,
            },
            transformed: Arc::new(plan),
        };
        // The proposal a defective producer would emit: the victim compare
        // removed even though the operand registers it reads were rewritten
        // after the shadow — the same register identities no longer name
        // the same values.
        let mut proposed = source.transformed().clone();
        proposed.functions[0].blocks[0].instructions.remove(3);
        let budget = OptimizationWorkBudget::new(100, 100, 1000, 100, 100).unwrap();
        assert_eq!(
            validate_redundant_compare(&source, 0, COMPARE, &environment, budget, proposed)
                .unwrap_err(),
            RedundantCompareError::UnsupportedUse
        );
    }
}

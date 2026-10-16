//! Fixed views constrain operation occurrences, not semantic value lifetimes.
//!
//! Move eligible fixed operands (and their tied components) in the target
//! copy bank onto instruction-local carriers. Other banks retain their original
//! operands for ordinary allocation. Every operation keeps its complete constraint;
//! target-admitted copies establish reads before it and publish writes after
//! it. CFG edges and source fuel remain unchanged, so joins still require one
//! common resident home on every copy-free incoming edge.
//!
//! Occurrence analysis selects fixed/tied components, not every semantic
//! register in a large aggregate. It still observes every reference, escape
//! and intervening operation of each selected register. Unselected registers
//! keep their already-validated identities and ordinary allocation route.

use std::sync::Arc;

use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterClassId, RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedInstructionPlanIdentity, SelectedOperand, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{FuelScheduleIdentity, ScalarType};

use crate::ValidatedSelectedAnalysis;
use crate::rewrites::runtime_spill::control_mut;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFixedOccurrences {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: FixedOccurrencesReceipt,
}

validated_transformed_receipt!(ValidatedFixedOccurrences, FixedOccurrencesReceipt);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedOccurrencesReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

receipt_selected_identities!(FixedOccurrencesReceipt);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedOccurrencesError {
    SourceMismatch,
    UnsupportedCarrier,
    ConstraintMismatch,
    CopyConstraintMismatch {
        register: VirtualRegisterId,
        class: RegisterClassId,
    },
    IdentityOverflow,
    WorkBudgetExceeded {
        function: usize,
        required: usize,
        limit: u64,
        registers: usize,
        operands: usize,
        transports: usize,
        tie_steps: usize,
        settlement_steps: usize,
    },
}

impl std::fmt::Display for FixedOccurrencesError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "fixed-occurrence materialization failed: {self:?}"
        )
    }
}

impl std::error::Error for FixedOccurrencesError {}

/// Materialize eligible physical occurrences through the target's ordinary copy row.
/// This is a required realization, not an optimizer policy or a permission
/// to change the operation's fixed views, ties, clobbers, or scalar types.
pub fn materialize_selected_fixed_occurrences(
    source: &impl ValidatedSelectedAnalysis,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedFixedOccurrences, FixedOccurrencesError> {
    if source.selected_plan().target != environment.target()
        || source.selected_plan().fuel_schedule != source.fuel_schedule_identity()
    {
        return Err(FixedOccurrencesError::SourceMismatch);
    }
    let row = environment
        .constraint(environment.selected_keys().copy_i64)
        .ok_or(FixedOccurrencesError::ConstraintMismatch)?;
    let mut transformed = source.selected_plan().clone();
    for (function_index, function) in transformed.functions.iter_mut().enumerate() {
        let operand_count = function
            .instructions()
            .chain(
                function
                    .blocks
                    .iter()
                    .map(|block| selected_instructions::terminator_instruction(&block.terminator)),
            )
            .try_fold(0usize, |total, instruction| {
                total.checked_add(instruction.operands.len())
            })
            .ok_or(FixedOccurrencesError::IdentityOverflow)?;
        let transport_count =
            super::function_transports(function).ok_or(FixedOccurrencesError::IdentityOverflow)?;
        // Only a row containing a fixed operand can contribute a constrained
        // component. Its complete arity bounds tie closure's selected roster
        // before doing that work, including duplicates. Charge the selection
        // and lookups against this bound, never an unrelated aggregate's size.
        let constrained_operand_bound = function
            .instructions()
            .chain(
                function
                    .blocks
                    .iter()
                    .map(|block| selected_instructions::terminator_instruction(&block.terminator)),
            )
            .try_fold(0usize, |total, instruction| {
                total.checked_add(
                    if instruction
                        .operands
                        .iter()
                        .any(|operand| operand.fixed_view.is_some())
                    {
                        instruction.operands.len()
                    } else {
                        0
                    },
                )
            })
            .ok_or(FixedOccurrencesError::IdentityOverflow)?;
        let register_depth =
            usize::BITS as usize - constrained_operand_bound.max(1).leading_zeros() as usize;
        let tie_steps = function
            .instructions()
            .chain(
                function
                    .blocks
                    .iter()
                    .map(|block| selected_instructions::terminator_instruction(&block.terminator)),
            )
            .try_fold(0usize, |total, instruction| {
                let arity = instruction.operands.len();
                let ties = instruction
                    .operands
                    .iter()
                    .filter(|operand| operand.tied_to.is_some())
                    .count();
                // Untied wide call rows need one scan, not a cubic search.
                // Each tie can search an arity-wide row for at most arity+1
                // closure rounds; the row scans fit the same bound.
                total.checked_add(
                    arity.checked_mul(2)?.checked_add(
                        ties.checked_mul(arity)?
                            .checked_mul(arity.checked_add(1)?)?
                            .checked_mul(2)?,
                    )?,
                )
            })
            .ok_or(FixedOccurrencesError::IdentityOverflow)?;
        let settlement_steps = function
            .blocks
            .len()
            .checked_mul(function.boundary_settlements.len())
            .ok_or(FixedOccurrencesError::IdentityOverflow)?;
        // Charge each work domain separately. Scan the source register roster
        // once to join the selected identities; only selected rows are sorted.
        // Every operand performs a lookup in that smaller roster, retaining
        // all references of its selected register, not just fixed occurrences.
        // Four additional operand steps cover retaining and consuming those
        // slots, and two instruction scans cover their construction/validation.
        // Each transport names at most two registers. Tie closure runs once.
        let steps = super::function_scan(function)
            .and_then(|scan| scan.checked_mul(10))
            .and_then(|steps| {
                steps.checked_add(operand_count.checked_mul(register_depth.checked_add(16)?)?)
            })
            .and_then(|steps| {
                steps.checked_add(
                    transport_count.checked_mul(register_depth.checked_mul(2)?.checked_add(4)?)?,
                )
            })
            .and_then(|steps| {
                steps.checked_add(
                    function
                        .virtual_registers
                        .len()
                        .checked_mul(register_depth.checked_add(4)?)?,
                )
            })
            .and_then(|steps| {
                steps.checked_add(
                    constrained_operand_bound
                        .checked_mul(register_depth.checked_mul(4)?.checked_add(12)?)?,
                )
            })
            .and_then(|steps| steps.checked_add(tie_steps))
            .and_then(|steps| steps.checked_add(settlement_steps))
            .and_then(|steps| steps.checked_add(source.selected_plan().functions.len()))
            .ok_or(FixedOccurrencesError::IdentityOverflow)?;
        let required_budget =
            super::required_rewrite_budget(source.selected_plan(), function, budget)
                .ok_or(FixedOccurrencesError::IdentityOverflow)?;
        super::charge_steps(
            steps,
            required_budget,
            || FixedOccurrencesError::IdentityOverflow,
            || FixedOccurrencesError::WorkBudgetExceeded {
                function: function_index,
                required: steps,
                limit: required_budget.validation_steps(),
                registers: function.virtual_registers.len(),
                operands: operand_count,
                transports: transport_count,
                tie_steps,
                settlement_steps,
            },
        )?;
        rewrite_function(function, row)?;
    }
    Ok(super::sealed!(
        ValidatedFixedOccurrences,
        FixedOccurrencesReceipt,
        source,
        transformed
    ))
}

fn rewrite_function(
    function: &mut SelectedFunction,
    row: &RegisterInstructionConstraint,
) -> Result<(), FixedOccurrencesError> {
    let mut next_register = function
        .virtual_registers
        .iter()
        .map(|register| register.id.0)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(FixedOccurrencesError::IdentityOverflow)?;
    let mut next_instruction = function
        .instructions()
        .map(|instruction| instruction.id.0)
        .chain(function.blocks.iter().map(|block| {
            selected_instructions::terminator_instruction(&block.terminator)
                .id
                .0
        }))
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(FixedOccurrencesError::IdentityOverflow)?;
    let occurrences = local_occurrence_registers(function)?;
    let mut prepared_instructions = occurrences.instructions.iter();
    let mut registers = function.virtual_registers.clone();
    for block in &mut function.blocks {
        let original = std::mem::take(&mut block.instructions);
        let mut positions = Vec::with_capacity(original.len() + 1);
        for mut instruction in original {
            positions.push(
                u32::try_from(block.instructions.len())
                    .map_err(|_| FixedOccurrencesError::IdentityOverflow)?,
            );
            let (before, after) = materialize(
                &mut instruction,
                &mut registers,
                &occurrences,
                prepared_instructions
                    .next()
                    .ok_or(FixedOccurrencesError::SourceMismatch)?,
                row,
                &mut next_register,
                &mut next_instruction,
            )?;
            block.instructions.extend(before);
            block.instructions.push(instruction);
            block.instructions.extend(after);
        }
        positions.push(
            u32::try_from(block.instructions.len())
                .map_err(|_| FixedOccurrencesError::IdentityOverflow)?,
        );
        for settlement in &mut function.boundary_settlements {
            if settlement.block == block.id {
                settlement.instruction_index = *positions
                    .get(settlement.instruction_index as usize)
                    .ok_or(FixedOccurrencesError::SourceMismatch)?;
            }
        }
        let (before, after) = materialize(
            control_mut(&mut block.terminator),
            &mut registers,
            &occurrences,
            prepared_instructions
                .next()
                .ok_or(FixedOccurrencesError::SourceMismatch)?,
            row,
            &mut next_register,
            &mut next_instruction,
        )?;
        if !after.is_empty() {
            return Err(FixedOccurrencesError::ConstraintMismatch);
        }
        block.instructions.extend(before);
    }
    if prepared_instructions.next().is_some() {
        return Err(FixedOccurrencesError::SourceMismatch);
    }
    function.virtual_registers = registers;
    Ok(())
}

fn materialize(
    instruction: &mut SelectedInstruction,
    registers: &mut Vec<VirtualRegister>,
    occurrences: &OccurrencePlan,
    prepared: &PreparedInstruction,
    row: &RegisterInstructionConstraint,
    next_register: &mut u32,
    next_instruction: &mut u32,
) -> Result<(Vec<SelectedInstruction>, Vec<SelectedInstruction>), FixedOccurrencesError> {
    // Match the original occurrence before inserting any copies. Prepared
    // slots index the stable source roster, never the growing register vector.
    if instruction.id != prepared.id || instruction.operands.len() != prepared.operands.len() {
        return Err(FixedOccurrencesError::SourceMismatch);
    }
    let mut before = Vec::new();
    let mut after = Vec::new();
    // A read carrier places this occurrence's fixed operand, so it belongs to
    // the occurrence's operations: a returning foreign call requires every
    // instruction between its floating-control save and the call to carry the
    // call's operation. A write carrier preserves the published result and
    // stays an operation-free value copy, the shape result transport follows.
    let operations = instruction.provenance.operations.clone();
    for (operand, roster_index) in instruction.operands.iter_mut().zip(&prepared.operands) {
        let Some(roster_index) = *roster_index else {
            continue;
        };
        let occurrence = occurrences
            .registers
            .get(roster_index)
            .ok_or(FixedOccurrencesError::SourceMismatch)?;
        if occurrence.id != operand.virtual_register {
            return Err(FixedOccurrencesError::SourceMismatch);
        }
        if occurrence.instruction_local {
            continue;
        }
        let original_index = occurrence.source_index;
        let original = registers[original_index].clone();
        // Scratch already is an operation-local target temporary. It has no
        // semantic payload and must never acquire source-copy authority.
        if matches!(
            original.origin,
            VirtualRegisterOrigin::InstructionScratch { .. }
        ) {
            continue;
        }
        if operand.class != original.class {
            return Err(FixedOccurrencesError::SourceMismatch);
        }
        let copy_class = row
            .operands
            .first()
            .ok_or(FixedOccurrencesError::ConstraintMismatch)?
            .class;
        // This rewrite has authority only for the target copy bank. Floating
        // ABI carriers keep their original rows and pass through ordinary
        // allocation; floating bit values in the integer bank remain eligible.
        if original.class != copy_class {
            continue;
        }
        admit_copy(row, &original)?;
        let fresh = VirtualRegisterId(take_identity(next_register)?);
        let mut temporary = original.clone();
        temporary.id = fresh;
        temporary.entry_fixed_view = None;
        let reads = matches!(
            operand.access,
            RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
        );
        let writes = matches!(
            operand.access,
            RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
        );
        if reads {
            let id = SelectedInstructionId(take_identity(next_instruction)?);
            before.push(copy(row, id, original.id, fresh, &original, &operations));
            temporary.origin = origin_at(original.origin, id)?;
        } else {
            temporary.origin = origin_at(original.origin, instruction.id)?;
        }
        if writes {
            let id = SelectedInstructionId(take_identity(next_instruction)?);
            after.push(copy(row, id, fresh, original.id, &original, &[]));
            registers[original_index].origin = origin_at(original.origin, id)?;
        }
        registers.push(temporary);
        operand.virtual_register = fresh;
    }
    Ok((before, after))
}

fn fixed_component(instruction: &SelectedInstruction) -> Result<Vec<bool>, FixedOccurrencesError> {
    // A tie is symmetric for physical placement even though its row stores
    // one direction. Every member must become local when one member is fixed.
    let mut local: Vec<bool> = instruction
        .operands
        .iter()
        .map(|operand| operand.fixed_view.is_some())
        .collect();
    loop {
        let mut changed = false;
        for (index, operand) in instruction.operands.iter().enumerate() {
            if let Some(tied) = operand.tied_to {
                let other = instruction
                    .operands
                    .iter()
                    .position(|candidate| candidate.operand == tied)
                    .ok_or(FixedOccurrencesError::ConstraintMismatch)?;
                if local[index] != local[other] {
                    local[index] = true;
                    local[other] = true;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    Ok(local)
}

fn take_identity(next: &mut u32) -> Result<u32, FixedOccurrencesError> {
    let identity = *next;
    *next = next
        .checked_add(1)
        .ok_or(FixedOccurrencesError::IdentityOverflow)?;
    Ok(identity)
}

/// Existing ABI transfers and a previous materialization already establish
/// local carriers. Retain those transfers: a floating ABI carrier may belong
/// to a different class from the semantic value's integer bit carrier.
/// Every reference and outgoing edge participates in this test. A resident
/// used at two fixed operations, or transported across a block edge, is never
/// mistaken for an occurrence carrier.
struct OccurrencePlan {
    registers: Vec<PreparedOccurrenceRegister>,
    instructions: Vec<PreparedInstruction>,
}

struct PreparedInstruction {
    id: SelectedInstructionId,
    // One slot per original operand. Only fixed-component members need a
    // materialization lookup; all operands still participate in local-carrier
    // analysis. The sorted register roster keeps these indices stable.
    operands: Vec<Option<usize>>,
}

struct PreparedOccurrenceRegister {
    id: VirtualRegisterId,
    source_index: usize,
    instruction_local: bool,
}

fn local_occurrence_registers(
    function: &SelectedFunction,
) -> Result<OccurrencePlan, FixedOccurrencesError> {
    // The input's sealed selected-analysis owner already established all
    // semantic register references. This operation needs only the fixed/tied
    // subset, but it cannot decide locality from only those occurrences.
    let mut selected_registers = Vec::new();
    let mut instruction_components = Vec::new();
    for instruction in function.blocks.iter().flat_map(|block| {
        block.instructions.iter().chain(std::iter::once(
            selected_instructions::terminator_instruction(&block.terminator),
        ))
    }) {
        let components = fixed_component(instruction)?;
        selected_registers.extend(
            instruction
                .operands
                .iter()
                .zip(&components)
                .filter_map(|(operand, selected)| selected.then_some(operand.virtual_register)),
        );
        instruction_components.push((instruction.id, components));
    }
    selected_registers.sort_unstable();
    selected_registers.dedup();
    // References borrow the input. Construction storage retains at most four
    // per register: a fourth already proves it cannot be an ABI local carrier.
    let mut roster: Vec<_> = selected_registers
        .into_iter()
        .map(|identity| (identity, false, Vec::new(), None))
        .collect();
    for (source_index, register) in function.virtual_registers.iter().enumerate() {
        if let Ok(position) = roster.binary_search_by_key(&register.id, |entry| entry.0) {
            if roster[position].3.replace(source_index).is_some() {
                return Err(FixedOccurrencesError::SourceMismatch);
            }
            roster[position].1 = register.entry_fixed_view.is_some();
        }
    }
    let mut instruction_components = instruction_components.into_iter();
    let mut instructions = Vec::new();
    for (block_index, block) in function.blocks.iter().enumerate() {
        for successor in selected_instructions::terminator_successors(&block.terminator) {
            let mut escape = |register| {
                if let Ok(index) = roster.binary_search_by_key(&register, |entry| entry.0) {
                    roster[index].1 = true;
                }
            };
            super::runtime_split::visit_structural_registers(successor, &mut escape);
            for binding in &successor.bindings {
                if let selected_instructions::SelectedValueTransport::Registers {
                    argument,
                    parameter,
                } = binding.transport
                {
                    escape(argument);
                    escape(parameter);
                }
            }
        }
        let mut destructive_prefix = 0usize;
        for (position, instruction) in block
            .instructions
            .iter()
            .chain(std::iter::once(
                selected_instructions::terminator_instruction(&block.terminator),
            ))
            .enumerate()
        {
            let (identity, components) = instruction_components
                .next()
                .ok_or(FixedOccurrencesError::SourceMismatch)?;
            if identity != instruction.id {
                return Err(FixedOccurrencesError::SourceMismatch);
            }
            let mut prepared_operands = Vec::with_capacity(instruction.operands.len());
            for (operand_index, operand) in instruction.operands.iter().enumerate() {
                let Ok(index) =
                    roster.binary_search_by_key(&operand.virtual_register, |entry| entry.0)
                else {
                    prepared_operands.push(None);
                    continue;
                };
                prepared_operands.push(components[operand_index].then_some(index));
                if roster[index].2.len() < 4 {
                    roster[index].2.push((
                        block_index,
                        position,
                        instruction,
                        operand,
                        components[operand_index],
                        destructive_prefix,
                    ));
                }
            }
            instructions.push(PreparedInstruction {
                id: instruction.id,
                operands: prepared_operands,
            });
            destructive_prefix += usize::from(!transport_kind(instruction.kind));
        }
    }
    if instruction_components.next().is_some() {
        return Err(FixedOccurrencesError::SourceMismatch);
    }
    let mut registers = Vec::with_capacity(roster.len());
    for (register, escapes, references, index) in roster {
        registers.push(PreparedOccurrenceRegister {
            id: register,
            source_index: index.ok_or(FixedOccurrencesError::SourceMismatch)?,
            instruction_local: false,
        });
        if escapes || references.is_empty() || references.len() > 3 {
            continue;
        }
        let mut constrained = references.iter().filter(|reference| reference.4);
        let Some(occurrence) = constrained.next() else {
            continue;
        };
        if constrained.next().is_some()
            || references
                .iter()
                .any(|reference| reference.0 != occurrence.0)
        {
            continue;
        }
        let first = &references[0];
        let last = &references[references.len() - 1];
        // Prefix differences inspect the complete gap, including operations
        // with no reference to this register. Only the fixed operation itself
        // and admitted transports may lie inside its occurrence window.
        if first.1 < last.1 {
            let between = last.5 - first.5 - usize::from(!transport_kind(first.2.kind));
            let occurrence_inside = usize::from(
                occurrence.1 > first.1
                    && occurrence.1 < last.1
                    && !transport_kind(occurrence.2.kind),
            );
            if between != occurrence_inside {
                continue;
            }
        }
        let compatible = references.iter().all(|reference| {
            reference.4
                || transport_kind(reference.2.kind)
                || (occurrence.3.access == RegisterOperandAccess::Use
                    && reference.3.access == RegisterOperandAccess::Def
                    && reference.1 < occurrence.1
                    && reference.3.fixed_view.is_none()
                    && reference.3.tied_to.is_none()
                    && !reference.3.early_clobber)
        });
        if compatible && (references.len() > 1 || occurrence.3.access == RegisterOperandAccess::Def)
        {
            registers
                .last_mut()
                .ok_or(FixedOccurrencesError::SourceMismatch)?
                .instruction_local = true;
        }
    }
    Ok(OccurrencePlan {
        registers,
        instructions,
    })
}

fn transport_kind(kind: SelectedInstructionKind) -> bool {
    matches!(
        kind,
        SelectedInstructionKind::CopyI64
            | SelectedInstructionKind::Float32ToBits
            | SelectedInstructionKind::Float64ToBits
            | SelectedInstructionKind::BitsToFloat32
            | SelectedInstructionKind::BitsToFloat64
            | SelectedInstructionKind::ZeroExtendU8
            | SelectedInstructionKind::ZeroExtendU16
            | SelectedInstructionKind::ZeroExtendU32
            | SelectedInstructionKind::SignExtendI8
            | SelectedInstructionKind::SignExtendI16
            | SelectedInstructionKind::SignExtendI32
    )
}

fn admit_copy(
    row: &RegisterInstructionConstraint,
    register: &VirtualRegister,
) -> Result<(), FixedOccurrencesError> {
    let payload = match register.scalar_type {
        ScalarType::Boolean => true,
        ScalarType::Integer(integer) => {
            // A physical copy preserves the exact scalar carrier, including
            // addr. It neither dereferences it nor grants backing authority.
            matches!(integer.bits(), 8 | 16 | 32 | 64)
        }
        ScalarType::IeeeFloat(_) => true,
    };
    if !payload {
        return Err(FixedOccurrencesError::UnsupportedCarrier);
    }
    let flexible = |operand: &register_model::RegisterOperandConstraint, access| {
        operand.class == register.class
            && operand.access == access
            && operand.fixed_view.is_none()
            && operand.tied_to.is_none()
            && !operand.early_clobber
    };
    if row.operands.len() != 2
        || !flexible(&row.operands[0], RegisterOperandAccess::Use)
        || !flexible(&row.operands[1], RegisterOperandAccess::Def)
        || !row.implicit_uses.is_empty()
        || !row.implicit_defs.is_empty()
        || !row.clobbers.is_empty()
    {
        return Err(FixedOccurrencesError::CopyConstraintMismatch {
            register: register.id,
            class: register.class,
        });
    }
    Ok(())
}

fn origin_at(
    origin: VirtualRegisterOrigin,
    instruction: SelectedInstructionId,
) -> Result<VirtualRegisterOrigin, FixedOccurrencesError> {
    Ok(match origin {
        VirtualRegisterOrigin::InstructionResult { source_value, .. }
        | VirtualRegisterOrigin::EntryParameter { source_value, .. }
        | VirtualRegisterOrigin::BlockParameter { source_value, .. } => {
            VirtualRegisterOrigin::InstructionResult {
                instruction,
                source_value,
            }
        }
        VirtualRegisterOrigin::StructuralObservation {
            place, byte_offset, ..
        }
        | VirtualRegisterOrigin::AbiTransport {
            place, byte_offset, ..
        } => VirtualRegisterOrigin::AbiTransport {
            instruction,
            place,
            byte_offset,
        },
        _ => return Err(FixedOccurrencesError::UnsupportedCarrier),
    })
}

fn copy(
    row: &RegisterInstructionConstraint,
    id: SelectedInstructionId,
    input: VirtualRegisterId,
    output: VirtualRegisterId,
    register: &VirtualRegister,
    operations: &[semantic_vocabulary::OperationId],
) -> SelectedInstruction {
    let operand = |constraint: &register_model::RegisterOperandConstraint, virtual_register| {
        SelectedOperand {
            operand: constraint.operand,
            virtual_register,
            access: constraint.access,
            class: constraint.class,
            fixed_view: constraint.fixed_view,
            tied_to: constraint.tied_to,
            early_clobber: constraint.early_clobber,
        }
    };
    let mut instruction = SelectedInstruction {
        id,
        kind: SelectedInstructionKind::CopyI64,
        constraint: row.key,
        operands: vec![
            operand(&row.operands[0], input),
            operand(&row.operands[1], output),
        ],
        implicit_uses: row.implicit_uses.clone(),
        implicit_defs: row.implicit_defs.clone(),
        clobbers: row.clobbers.clone(),
        provenance: selected_instructions::SelectedInstructionProvenance {
            operations: operations.to_vec(),
            ..Default::default()
        },
    };
    match register.origin {
        VirtualRegisterOrigin::InstructionResult { source_value, .. }
        | VirtualRegisterOrigin::EntryParameter { source_value, .. }
        | VirtualRegisterOrigin::BlockParameter { source_value, .. } => {
            instruction.provenance.values.push(source_value)
        }
        _ => {}
    }
    instruction
}

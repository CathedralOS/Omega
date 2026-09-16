use optimization_core::OptimizationWorkBudget;
use optimization_unit::ValueDefinitionSite;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{
    RegisterClassId, RegisterInstructionConstraint, RegisterOperandAccess, RegisterUnitId,
};
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlock, SelectedBlockOrigin,
    SelectedCasePayloadTransport, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedOperand, SelectedStructuralTransport, SelectedSuccessorRole,
    SelectedTerminator, SelectedValueTransport, VirtualRegister, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, ValueId};

use super::RuntimeSpillError;
use crate::ValidatedSelectedAnalysis;

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub use_blocks: Vec<usize>,
    /// Per block: one reload register serves every flexible use when a view of
    /// the victim's class survives everything the block touches. `false` keeps
    /// each use on a private reload pair.
    pub shared_reload: Vec<bool>,
    pub victim: &'source VirtualRegister,
    pub source_value: ValueId,
    pub address_scalar_type: ScalarType,
    pub definitions: Vec<StorageDefinition>,
    pub slot: LocalStorageSlotId,
    pub first_instruction: u32,
    pub first_register: u32,
    pub address: &'source RegisterInstructionConstraint,
    pub load: &'source RegisterInstructionConstraint,
    pub store: &'source RegisterInstructionConstraint,
}

/// Source definition coordinates, not proposed spill instructions. An incoming
/// parameter has one exact edge-copy definition per predecessor.
pub(super) struct StorageDefinition {
    pub block_index: usize,
    pub instruction: SelectedInstructionId,
    pub register: VirtualRegisterId,
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, RuntimeSpillError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(RuntimeSpillError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(RuntimeSpillError::SourceMismatch)?;
    let victim = function
        .virtual_registers
        .iter()
        .find(|value| value.id == register)
        .ok_or(RuntimeSpillError::UnsupportedValue)?;
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 64)
        .map_err(|_| RuntimeSpillError::UnsupportedValue)?;
    let (source_value, definition, block_index) = match victim.origin {
        VirtualRegisterOrigin::InstructionResult {
            instruction,
            source_value,
        } => {
            let block_index = function
                .blocks
                .iter()
                .position(|block| {
                    block
                        .instructions
                        .iter()
                        .any(|candidate| candidate.id == instruction)
                })
                .ok_or(RuntimeSpillError::UnsupportedValue)?;
            (source_value, Some(instruction), block_index)
        }
        VirtualRegisterOrigin::BlockParameter {
            source_value,
            block,
            parameter_index,
        } => {
            let block_index = function
                .blocks
                .iter()
                .position(|candidate| candidate.id == block)
                .ok_or(RuntimeSpillError::UnsupportedValue)?;
            if block == function.entry_block
                || !matches!(victim.definition_site,
                Some(ValueDefinitionSite::BlockParameter { block: semantic_block, position })
                    if semantic_block == function.blocks[block_index].source_block()
                        && position as usize == parameter_index)
            {
                return Err(RuntimeSpillError::UnsupportedValue);
            }
            (source_value, None, block_index)
        }
        _ => return Err(RuntimeSpillError::UnsupportedValue),
    };
    // This preserves a full GPR in its own eight-byte slot, not a source
    // referent. Narrow values keep their exact type and all resident bits;
    // neither signed widening nor a wider read of source storage is needed.
    let scalar_payload = match victim.scalar_type {
        ScalarType::Boolean | ScalarType::IeeeFloat(_) => true,
        ScalarType::Integer(integer) => {
            !integer.is_address() && matches!(integer.bits(), 8 | 16 | 32 | 64)
        }
    };
    // Semantic lineage alone does not locate physical storage definitions.
    // Instruction results and incoming parameters establish those separately.
    if !scalar_payload
        || victim.entry_fixed_view.is_some()
        || !matches!(
            victim.definition_site,
            Some(ValueDefinitionSite::FunctionParameter(_))
                | Some(ValueDefinitionSite::BlockParameter { .. })
                | Some(ValueDefinitionSite::Node { .. })
        )
    {
        return Err(RuntimeSpillError::UnsupportedValue);
    }
    let slot = LocalStorageSlotId::Spill { register };
    if function
        .local_storage_slots
        .iter()
        .any(|storage| storage.id == slot)
    {
        return Err(RuntimeSpillError::UnsupportedUse);
    }
    let definitions = if let Some(instruction) = definition {
        vec![StorageDefinition {
            block_index,
            instruction,
            register,
        }]
    } else {
        parameter_definitions(function, block_index, victim, source_value)?
    };
    let mut defined = definition.is_none();
    let mut uses = 0usize;
    let mut use_blocks = Vec::new();
    // Blocks where at least one admitted use can read a shared block-local
    // reload: an unpinned instruction or terminator operand, or an
    // edge-transport argument. ABI-pinned (fixed-view) uses keep a private
    // reload pinned for their own point instead.
    let mut flexible_uses = vec![false; function.blocks.len()];
    for (current_block_index, block) in function.blocks.iter().enumerate() {
        let previous_uses = uses;
        let (terminal, successors) = super::control(&block.terminator);
        // A terminator operand use executes after every block instruction, so
        // the same private reload serves it from the end of the block; the
        // dominance check below already covers that position. As on body
        // instruction operands, a fixed view stays attached and pins the fresh
        // reload register to the same physical unit, so ABI-pinned returns and
        // hosted exits stay exact. Tied, early-clobber, and defining
        // references stay rejected.
        for operand in &terminal.operands {
            if operand.virtual_register != register {
                continue;
            }
            match operand.access {
                RegisterOperandAccess::Use
                    if operand.tied_to.is_none()
                        && !operand.early_clobber
                        && operand.class == victim.class =>
                {
                    // An output tied to this use would extend the reload's
                    // value identity.
                    if terminal
                        .operands
                        .iter()
                        .any(|other| other.tied_to == Some(operand.operand))
                    {
                        return Err(RuntimeSpillError::UnsupportedUse);
                    }
                    if operand.fixed_view.is_none() {
                        flexible_uses[current_block_index] = true;
                    }
                    uses = uses
                        .checked_add(1)
                        .ok_or(RuntimeSpillError::IdentityOverflow)?;
                }
                _ => return Err(RuntimeSpillError::UnsupportedUse),
            }
        }
        for successor in successors.into_iter().flatten() {
            for binding in &successor.bindings {
                let SelectedValueTransport::Registers {
                    argument,
                    parameter,
                } = binding.transport
                else {
                    continue;
                };
                // The parameter side is the destination's incoming definition,
                // never a use in this block. Only a block-parameter victim's
                // own incoming edges may carry it; parameter_definitions has
                // already checked every such arrival.
                if parameter == register
                    && (definition.is_some() || successor.block != function.blocks[block_index].id)
                {
                    return Err(RuntimeSpillError::UnsupportedUse);
                }
                if argument != register {
                    continue;
                }
                // An edge-transport argument reads the victim at the end of
                // this block. Its semantic declaration must name the victim's
                // source value and exact type; anything else is an
                // inconsistent plan, not a use this rewrite can serve.
                if binding.semantic.argument != source_value
                    || binding.semantic.scalar_type != victim.scalar_type
                {
                    return Err(RuntimeSpillError::UnsupportedUse);
                }
                flexible_uses[current_block_index] = true;
                uses = uses
                    .checked_add(1)
                    .ok_or(RuntimeSpillError::IdentityOverflow)?;
            }
            if successor.structural_bindings.iter().any(|binding| {
                matches!(binding.transport,
                SelectedStructuralTransport::Descriptor { argument, .. }
                    | SelectedStructuralTransport::WholeValue { argument, .. }
                    if argument == register)
            }) {
                return Err(RuntimeSpillError::UnsupportedUse);
            }
            if let Some(case) = &successor.structural_case {
                for payload in &case.payloads {
                    match payload.transport {
                        SelectedCasePayloadTransport::Registers {
                            argument,
                            parameter,
                        } => {
                            // The parameter side is the destination's payload
                            // definition, never a use in this block — the same
                            // rule the value bindings above keep.
                            if parameter == register {
                                return Err(RuntimeSpillError::UnsupportedUse);
                            }
                            if argument != register {
                                continue;
                            }
                            // A case-payload register transport reads the
                            // victim at this edge, like the value bindings.
                            // Its declared payload type must equal the
                            // victim's exact type; a mismatched plan is not a
                            // use this rewrite can serve.
                            if payload.semantic.parameter.scalar_type != victim.scalar_type {
                                return Err(RuntimeSpillError::UnsupportedUse);
                            }
                            flexible_uses[current_block_index] = true;
                            uses = uses
                                .checked_add(1)
                                .ok_or(RuntimeSpillError::IdentityOverflow)?;
                        }
                        SelectedCasePayloadTransport::Unmaterialized { parameter }
                            if parameter == register =>
                        {
                            return Err(RuntimeSpillError::UnsupportedUse);
                        }
                        _ => {}
                    }
                }
            }
        }
        for instruction in &block.instructions {
            for operand in &instruction.operands {
                if operand.virtual_register != register {
                    continue;
                }
                match operand.access {
                    RegisterOperandAccess::Def
                        if !defined
                            && current_block_index == block_index
                            && Some(instruction.id) == definition =>
                    {
                        if matches!(
                            instruction.kind,
                            SelectedInstructionKind::FrameAddress { .. }
                                | SelectedInstructionKind::AddressOffset { .. }
                                | SelectedInstructionKind::ByteViewAddress
                        ) {
                            return Err(RuntimeSpillError::UnsupportedValue);
                        }
                        defined = true;
                    }
                    RegisterOperandAccess::Use
                        if (defined || current_block_index != block_index)
                            && Some(instruction.id) != definition
                            && operand.tied_to.is_none()
                            && !operand.early_clobber
                            && operand.class == victim.class =>
                    {
                        // A fixed view on this operand stays attached to the
                        // rewritten use, so the fresh reload register is a
                        // precolored segment pinned to that physical view for
                        // exactly the load-to-use window — the split the
                        // operand always needed, created by recovery instead
                        // of refusing the victim. An output tied to this use
                        // would extend the reload's value identity.
                        if instruction
                            .operands
                            .iter()
                            .any(|other| other.tied_to == Some(operand.operand))
                        {
                            return Err(RuntimeSpillError::UnsupportedUse);
                        }
                        if operand.fixed_view.is_none() {
                            flexible_uses[current_block_index] = true;
                        }
                        uses = uses
                            .checked_add(1)
                            .ok_or(RuntimeSpillError::IdentityOverflow)?;
                    }
                    _ => return Err(RuntimeSpillError::UnsupportedUse),
                }
            }
        }
        if uses != previous_uses {
            use_blocks.push(current_block_index);
        }
    }
    if !defined || uses == 0 {
        return Err(RuntimeSpillError::UnsupportedValue);
    }
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| total.checked_add(uses.checked_mul(4)?))
        .and_then(|total| total.checked_add(definitions.len()))
        .and_then(|total| total.checked_add(function.blocks.len().checked_mul(2)?))
        .ok_or(RuntimeSpillError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| RuntimeSpillError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(RuntimeSpillError::WorkBudgetExceeded);
    }
    super::require_dominated_uses(function, block_index, &use_blocks)?;
    let keys = environment.selected_keys();
    let address = environment
        .constraint(
            keys.frame_address
                .ok_or(RuntimeSpillError::ConstraintMismatch)?,
        )
        .ok_or(RuntimeSpillError::ConstraintMismatch)?;
    let load = environment
        .constraint(keys.load64.ok_or(RuntimeSpillError::ConstraintMismatch)?)
        .ok_or(RuntimeSpillError::ConstraintMismatch)?;
    let store = environment
        .constraint(keys.store64.ok_or(RuntimeSpillError::ConstraintMismatch)?)
        .ok_or(RuntimeSpillError::ConstraintMismatch)?;
    // Frame accesses retain the validated target row's stack-pointer reads.
    // Ordinary pointer loads have only their explicit address operand.
    if !load.implicit_uses.is_empty() {
        return Err(RuntimeSpillError::ConstraintMismatch);
    }
    for (row, accesses) in [
        (address, &[RegisterOperandAccess::Def][..]),
        (
            load,
            &[RegisterOperandAccess::Use, RegisterOperandAccess::Def][..],
        ),
        (store, &[RegisterOperandAccess::Use][..]),
    ] {
        if row.operands.len() != accesses.len()
            || !row.implicit_defs.is_empty()
            || !row.clobbers.is_empty()
        {
            return Err(RuntimeSpillError::ConstraintMismatch);
        }
        for (ordinal, (operand, access)) in row.operands.iter().zip(accesses).enumerate() {
            if usize::from(operand.operand) != ordinal
                || operand.access != *access
                || operand.class != victim.class
                || operand.fixed_view.is_some()
                || operand.tied_to.is_some()
                || operand.early_clobber
            {
                return Err(RuntimeSpillError::ConstraintMismatch);
            }
        }
    }
    let first_instruction = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .map(|instruction| instruction.id.0)
                .chain([super::control(&block.terminator).0.id.0])
        })
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(RuntimeSpillError::IdentityOverflow)?;
    let first_register = function
        .virtual_registers
        .iter()
        .map(|value| value.id.0)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(RuntimeSpillError::IdentityOverflow)?;
    // A unit implicitly used anywhere in the function can be live through this
    // block without appearing in the block's own effects: backward unit
    // liveness keeps it resident at every interior point, so a view containing
    // it cannot host an interval spanning the block. The frame rows the
    // rewrite inserts contribute their own implicit reads at interior points
    // inside a shared interval, so their units are excluded the same way. A
    // fixed-view operand or entry-bound register anywhere in the function
    // precolors an interval that may reach across this block, so every unit
    // such a view occupies is excluded too.
    let mut implicit_use_units = std::collections::BTreeSet::new();
    let mut pinned_views = function
        .virtual_registers
        .iter()
        .filter_map(|register| register.entry_fixed_view)
        .collect::<Vec<_>>();
    for instruction in function.blocks.iter().flat_map(|block| {
        block
            .instructions
            .iter()
            .chain(std::iter::once(super::control(&block.terminator).0))
    }) {
        implicit_use_units.extend(instruction.implicit_uses.iter().copied());
        pinned_views.extend(
            instruction
                .operands
                .iter()
                .filter_map(|operand| operand.fixed_view),
        );
    }
    implicit_use_units.extend(address.implicit_uses.iter().copied());
    implicit_use_units.extend(load.implicit_uses.iter().copied());
    implicit_use_units.extend(store.implicit_uses.iter().copied());
    let pinned_units = pinned_views
        .iter()
        .flat_map(|view_id| {
            environment
                .physical()
                .model()
                .views
                .get(usize::from(view_id.0))
                .into_iter()
                .flat_map(|view| view.units.iter().chain(&view.write_units))
        })
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let shared_reload = function
        .blocks
        .iter()
        .enumerate()
        .map(|(block_index, block)| {
            flexible_uses[block_index]
                && surviving_home_exists(
                    environment,
                    block,
                    victim.class,
                    &implicit_use_units,
                    &pinned_units,
                )
        })
        .collect();
    Ok(Admission {
        function,
        use_blocks,
        shared_reload,
        victim,
        source_value,
        address_scalar_type: ScalarType::Integer(unsigned),
        definitions,
        slot,
        first_instruction,
        first_register,
        address,
        load,
        store,
    })
}

fn parameter_definitions(
    function: &SelectedFunction,
    destination: usize,
    victim: &VirtualRegister,
    source_value: ValueId,
) -> Result<Vec<StorageDefinition>, RuntimeSpillError> {
    let mut definitions = Vec::new();
    for (block_index, block) in function.blocks.iter().enumerate() {
        for successor in super::control(&block.terminator).1.into_iter().flatten() {
            if successor.block != function.blocks[destination].id {
                continue;
            }
            // Every arrival must initialize the slot on its exact edge. Do not
            // place a store on a conditional predecessor shared by other paths.
            if successor.role != SelectedSuccessorRole::EdgeTransferContinuation
                || successor.structural_case.is_some()
                || !matches!(block.terminator, SelectedTerminator::Jump { .. })
                || !matches!(block.origin, SelectedBlockOrigin::EdgeTransfer { edge, target }
                    if edge == successor.psi_edge && target == successor.source_target)
            {
                return Err(RuntimeSpillError::UnsupportedControlFlow);
            }
            let mut bindings = successor.bindings.iter().filter(|binding| {
                matches!(binding.transport,
                SelectedValueTransport::Registers { parameter, .. } if parameter == victim.id)
            });
            let binding = bindings.next().ok_or(RuntimeSpillError::UnsupportedUse)?;
            if bindings.next().is_some()
                || binding.semantic.parameter != source_value
                || binding.semantic.scalar_type != victim.scalar_type
            {
                return Err(RuntimeSpillError::UnsupportedUse);
            }
            let SelectedValueTransport::Registers { argument, .. } = binding.transport else {
                return Err(RuntimeSpillError::UnsupportedUse);
            };
            let value = function
                .virtual_registers
                .iter()
                .find(|value| value.id == argument)
                .ok_or(RuntimeSpillError::UnsupportedValue)?;
            if value.scalar_type != victim.scalar_type || value.class != victim.class {
                return Err(RuntimeSpillError::UnsupportedValue);
            }
            let VirtualRegisterOrigin::InstructionResult {
                instruction,
                source_value: argument_value,
            } = value.origin
            else {
                return Err(RuntimeSpillError::UnsupportedValue);
            };
            if argument_value != binding.semantic.argument {
                return Err(RuntimeSpillError::UnsupportedValue);
            }
            let mut physical_definitions =
                function
                    .blocks
                    .iter()
                    .enumerate()
                    .flat_map(|(owner, block)| {
                        block.instructions.iter().flat_map(move |candidate| {
                            candidate
                                .operands
                                .iter()
                                .filter(move |operand| {
                                    operand.virtual_register == argument
                                        && operand.access != RegisterOperandAccess::Use
                                })
                                .map(move |_| (owner, candidate))
                        })
                    });
            let Some((owner, copy)) = physical_definitions.next() else {
                return Err(RuntimeSpillError::UnsupportedValue);
            };
            if physical_definitions.next().is_some()
                || owner != block_index
                || copy.id != instruction
                || copy.kind != SelectedInstructionKind::CopyI64
                || !copy.operands.iter().any(|operand| {
                    operand.virtual_register == argument
                        && operand.access == RegisterOperandAccess::Def
                })
            {
                return Err(RuntimeSpillError::UnsupportedUse);
            }
            definitions.push(StorageDefinition {
                block_index,
                instruction,
                register: argument,
            });
        }
    }
    if definitions.is_empty() {
        return Err(RuntimeSpillError::UnsupportedUse);
    }
    Ok(definitions)
}

pub(super) fn fresh(next: &mut u32) -> Result<u32, RuntimeSpillError> {
    let result = *next;
    *next = next
        .checked_add(1)
        .ok_or(RuntimeSpillError::IdentityOverflow)?;
    Ok(result)
}

/// The private address computation and load inserted for one admitted use, at
/// an instruction operand or at a terminator operand. Proposal and replay build
/// the identical pair from this one constructor; the consumer operand keeps its
/// own access, class, and any fixed ABI view while only the referenced register
/// changes.
pub(super) struct Reload {
    pub address_register: VirtualRegister,
    pub reload_register: VirtualRegister,
    pub address: SelectedInstruction,
    pub load: SelectedInstruction,
}

pub(super) fn reload(
    admitted: &Admission<'_>,
    register: VirtualRegisterId,
    next_instruction: &mut u32,
    next_register: &mut u32,
) -> Result<Reload, RuntimeSpillError> {
    let address_instruction = SelectedInstructionId(fresh(next_instruction)?);
    let load_instruction = SelectedInstructionId(fresh(next_instruction)?);
    let address_register = VirtualRegisterId(fresh(next_register)?);
    let reload_register = VirtualRegisterId(fresh(next_register)?);
    Ok(Reload {
        address_register: VirtualRegister {
            id: address_register,
            scalar_type: admitted.address_scalar_type,
            class: admitted.victim.class,
            origin: VirtualRegisterOrigin::SpillAddress {
                instruction: address_instruction,
                register,
            },
            definition_site: None,
            entry_fixed_view: None,
        },
        reload_register: VirtualRegister {
            id: reload_register,
            scalar_type: admitted.victim.scalar_type,
            class: admitted.victim.class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: load_instruction,
                source_value: admitted.source_value,
            },
            definition_site: admitted.victim.definition_site,
            entry_fixed_view: None,
        },
        address: instruction(
            address_instruction,
            SelectedInstructionKind::FrameAddress {
                slot: frame(admitted.slot),
                byte_offset: 0,
            },
            admitted.address,
            &[address_register],
        ),
        load: instruction(
            load_instruction,
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            admitted.load,
            &[address_register, reload_register],
        ),
    })
}

/// Target rows supply the complete operand/effect interface, not guessed ISA conventions.
pub(super) fn instruction(
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

pub(super) fn frame(slot: LocalStorageSlotId) -> FrameStorageSlotId {
    FrameStorageSlotId::Local(slot)
}

/// Whether one reload register can keep a legal home across every flexible
/// use in the block. The produced interval reaches from just before the first
/// flexible instruction-operand use through the last such use, so any unit the
/// block touches — clobber, implicit access — would conflict with it, as would
/// any unit still live through the block from an implicit use elsewhere (the
/// caller collects those into `implicit_use_units`) or precolored by a
/// fixed-view operand or entry-bound register anywhere in the function
/// (`pinned_units`). A view surviving all of that is exactly the callee-saved
/// home a call-straddling reload needs: a `CallUnit` clobbers every
/// caller-saved unit, so only callee-saved views qualify there.
///
/// Scanning the whole block is deliberately conservative: the plan carries no
/// unit-level liveness, so the check covers the full span a shared reload
/// could occupy rather than just the observed uses. Where no view survives,
/// every use keeps a private reload pair — the shape the rewrite always
/// produced — and no produced interval is forced across a call.
fn surviving_home_exists(
    environment: &ValidatedTargetRegisterEnvironment,
    block: &SelectedBlock,
    class: RegisterClassId,
    implicit_use_units: &std::collections::BTreeSet<RegisterUnitId>,
    pinned_units: &std::collections::BTreeSet<RegisterUnitId>,
) -> bool {
    let reserved = environment.reservations().reserved_units();
    let mut touched = std::collections::BTreeSet::new();
    for instruction in block
        .instructions
        .iter()
        .chain(std::iter::once(super::control(&block.terminator).0))
    {
        touched.extend(instruction.clobbers.iter().copied());
        touched.extend(instruction.implicit_uses.iter().copied());
        touched.extend(instruction.implicit_defs.iter().copied());
    }
    let physical = environment.physical().model();
    physical
        .classes
        .iter()
        .find(|row| row.id == class)
        .is_some_and(|row| {
            row.views.iter().any(|view_id| {
                physical
                    .views
                    .get(usize::from(view_id.0))
                    .is_some_and(|view| {
                        view.id == *view_id
                            && view.allocatable
                            && view.units.iter().chain(&view.write_units).all(|unit| {
                                !touched.contains(unit)
                                    && !implicit_use_units.contains(unit)
                                    && !pinned_units.contains(unit)
                                    && reserved.binary_search(unit).is_err()
                            })
                    })
            })
        })
}

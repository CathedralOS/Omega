//! Producer admission of a copied-call-operand pair.
//!
//! The descriptor table drives the admission: the consumer instruction's
//! semantic kind selects the declared call consumers, the named producer
//! completes the pair, and the rule's axes — operand roster, unit surface,
//! effect surface — state every gate the concrete records must satisfy.
//! What no catalog declaration can attest is checked on the function
//! itself: the producer's identity as the destination register's last
//! definition before the call, the source register's freedom from
//! intervening redefinition, the operand classes against the roster, and
//! the whole-function work all charge into the caller's budget. The replay
//! in `replay` re-derives all of it without the table.

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    MachineEffectDeclaration, MachineSemanticKind, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, ValidatedMachineEffectCatalog,
    VirtualRegisterId,
};

use super::CopiedCallOperandError;
use super::pair::{copied_call_operand_for, declared_consumer};
use crate::ValidatedSelectedAnalysis;
use crate::machine_semantic_kind;

/// One admitted copied-call-operand pair: everything `rewrite` needs to
/// rebuild the consumer instruction and everything `replay` needs to
/// re-check it.
pub(super) struct AdmittedPair<'source> {
    pub(super) function: &'source SelectedFunction,
    pub(super) block_index: usize,
    /// The consumer's position in its block's instruction stream.
    pub(super) position: usize,
    /// The producer's source register: the call's `Use` operands that named
    /// the destination rebind to it.
    pub(super) source: VirtualRegisterId,
    /// The producer's destination register: the register the call's `Use`
    /// operands named before the reroute.
    pub(super) destination: VirtualRegisterId,
}

/// The instruction defining `register` at `position`: the last instruction
/// in the block before it carrying a non-`Use` operand on that register.
/// Blocks execute in order, so that definition is the value the call
/// observes; edge and entry bindings can only reach it when no body
/// instruction defines the register, in which case there is no in-block
/// `CopyI64` producer to fold.
fn last_definition_before(
    instructions: &[SelectedInstruction],
    position: usize,
    register: VirtualRegisterId,
) -> Option<usize> {
    instructions[..position].iter().rposition(|instruction| {
        instruction.operands.iter().any(|operand| {
            operand.access != RegisterOperandAccess::Use && operand.virtual_register == register
        })
    })
}

/// Admit `producer` and `call` — body instructions in `function_index`'s
/// function — as a copied-call-operand pair under the descriptor table.
///
/// The order of gates matters for the error vocabulary: the record shape
/// (consumer kind, operand roster, clean operand bindings) refuses first,
/// then the constraint-row contract, then the producer — a clean `CopyI64`
/// that is the destination's last in-block definition — and the source's
/// freedom from intervening redefinition, then the catalog declarations'
/// retained-call surface, and last the bounded-work charge.
pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    producer: SelectedInstructionId,
    call: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    effect_catalog: &ValidatedMachineEffectCatalog,
    budget: OptimizationWorkBudget,
) -> Result<AdmittedPair<'source>, CopiedCallOperandError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(CopiedCallOperandError::SourceMismatch);
    }
    // The bound catalog must describe this plan's target and this
    // environment's constraint catalog and selected-key inventory: a
    // foreign catalog's declarations cannot attest this program's encoded
    // surfaces even where constraint keys coincide numerically.
    let catalog = effect_catalog.catalog();
    if catalog.target != plan.target
        || catalog.register_constraints != environment.constraints().identity()
        || catalog.selected_keys != environment.selected_keys()
    {
        return Err(CopiedCallOperandError::EffectSurfaceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(CopiedCallOperandError::SourceMismatch)?;
    // The consumer is a body instruction: a direct call at a mid-block
    // position. A terminator's carried instruction is not a call form the
    // family declares.
    let (block_index, position, consumer) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .enumerate()
                .find(|(_, instruction)| instruction.id == call)
                .map(|(position, instruction)| (block_index, position, instruction))
        })
        .ok_or(CopiedCallOperandError::UnsupportedConsumer)?;
    if !declared_consumer(consumer.kind) {
        return Err(CopiedCallOperandError::UnsupportedConsumer);
    }
    let consumer_kind = machine_semantic_kind(consumer.kind);
    // The declared consumer kind names exactly one pair — every declared
    // rule shares the `CopyI64` producer — so the rule resolves before the
    // producer is even read.
    let rule = copied_call_operand_for(MachineSemanticKind::CopyI64, consumer_kind)
        .ok_or(CopiedCallOperandError::UnsupportedConsumer)?;
    debug_assert_eq!(
        rule.rewritten(),
        consumer_kind,
        "every declared copied-call-operand pair rewrites to the consumer's own semantic"
    );
    // The emitted call form is the dense `Use`-arguments/`Def`-results
    // roster the declared operand shape describes, under the
    // `CallSurfaceRetained` unit surface: `tied_to` and `early_clobber`
    // bindings are admitted nowhere, while implicit traffic and `fixed_view`
    // pins carry verbatim onto the rewritten record.
    if !rule.admits_consumer_operands(&consumer.operands) {
        return Err(CopiedCallOperandError::UnsupportedConsumer);
    }
    let find_register = |register| {
        function
            .virtual_registers
            .iter()
            .find(|entry| entry.id == register)
    };
    // Every operand's register must exist in the function's roster before
    // the row contract can even be read against it.
    if consumer
        .operands
        .iter()
        .any(|operand| find_register(operand.virtual_register).is_none())
    {
        return Err(CopiedCallOperandError::UnsupportedUse);
    }
    // The consumer's declared constraint row must publish the same operand
    // roster its records carry — the call contract's dense numbering,
    // leading `Use` arguments, trailing `Def` results, and the ABI
    // `fixed_view` pin on every operand — and the record must restate the
    // row's operand fields and implicit-unit surfaces verbatim: a different
    // row would change what each operand means, and a divergent implicit
    // list would silently republish a surface the call does not own. Each
    // operand's class must equal its roster row's class.
    let row = environment
        .constraint(consumer.constraint)
        .ok_or(CopiedCallOperandError::ConstraintMismatch)?;
    if row.operands.len() != consumer.operands.len()
        || !rule.admits_consumer_row(row)
        || row
            .operands
            .iter()
            .zip(&consumer.operands)
            .any(|(row_operand, operand)| {
                row_operand.operand != operand.operand
                    || row_operand.access != operand.access
                    || row_operand.class != operand.class
                    || row_operand.fixed_view != operand.fixed_view
                    || row_operand.tied_to != operand.tied_to
                    || row_operand.early_clobber != operand.early_clobber
            })
        || row.implicit_uses != consumer.implicit_uses
        || row.implicit_defs != consumer.implicit_defs
        || row.clobbers != consumer.clobbers
        || consumer.operands.iter().any(|operand| {
            find_register(operand.virtual_register)
                .is_some_and(|register| register.class != operand.class)
        })
    {
        return Err(CopiedCallOperandError::ConstraintMismatch);
    }
    // The named producer must be a body instruction in the consumer's
    // block ahead of it — a clean `CopyI64` `[use source, def destination]`
    // record under the register-only unit surface. A copy that rewrote its
    // own source (`source == destination`) leaves the rebound operand
    // reading the copied value, not the source — refusing rather than
    // rerouting an identity the copy does not establish.
    let producer_index = function.blocks[block_index].instructions[..position]
        .iter()
        .position(|instruction| instruction.id == producer)
        .ok_or(CopiedCallOperandError::UnsupportedProducer)?;
    let producer_instruction = &function.blocks[block_index].instructions[producer_index];
    if !rule.matches_producer(producer_instruction.kind)
        || !rule.admits_producer_record(producer_instruction)
    {
        return Err(CopiedCallOperandError::UnsupportedProducer);
    }
    let SelectedInstructionKind::CopyI64 = producer_instruction.kind else {
        return Err(CopiedCallOperandError::UnsupportedProducer);
    };
    let [source_operand, destination_operand] = producer_instruction.operands.as_slice() else {
        return Err(CopiedCallOperandError::UnsupportedProducer);
    };
    if source_operand.operand != 0
        || source_operand.access != RegisterOperandAccess::Use
        || destination_operand.operand != 1
        || destination_operand.access != RegisterOperandAccess::Def
    {
        return Err(CopiedCallOperandError::UnsupportedProducer);
    }
    let copied_source = source_operand.virtual_register;
    let destination = destination_operand.virtual_register;
    if copied_source == destination {
        return Err(CopiedCallOperandError::UnsupportedProducer);
    }
    // The producer must be the destination's last definition before the
    // call: it is the value every copied operand observes.
    if last_definition_before(
        &function.blocks[block_index].instructions,
        position,
        destination,
    ) != Some(producer_index)
    {
        return Err(CopiedCallOperandError::UnsupportedProducer);
    }
    // The call must actually consume the copy: at least one `Use` operand
    // reads the destination register.
    if !consumer.operands.iter().any(|operand| {
        operand.access == RegisterOperandAccess::Use && operand.virtual_register == destination
    }) {
        return Err(CopiedCallOperandError::UnsupportedUse);
    }
    let source_register =
        find_register(copied_source).ok_or(CopiedCallOperandError::UnsupportedUse)?;
    // The rebound operands keep the call operand's declared class, so the
    // source register must publish that class — and the producer's own row
    // must declare the `[use, def]` shape its operands carry at matching
    // classes: a copy whose operand classes differ forwards bits between
    // register classes, not the transparent value this pair names.
    let producer_row = environment
        .constraint(producer_instruction.constraint)
        .ok_or(CopiedCallOperandError::ConstraintMismatch)?;
    if producer_row.operands.len() != 2
        || producer_row.operands[0].operand != 0
        || producer_row.operands[0].access != RegisterOperandAccess::Use
        || producer_row.operands[0].class != source_operand.class
        || producer_row.operands[0].class != source_register.class
        || producer_row.operands[1].operand != 1
        || producer_row.operands[1].access != RegisterOperandAccess::Def
        || producer_row.operands[1].class != destination_operand.class
        || producer_row.operands[0].class != producer_row.operands[1].class
        || !rule.admits_producer_row(producer_row)
    {
        return Err(CopiedCallOperandError::ConstraintMismatch);
    }
    // Every operand the reroute rebinds keeps its declared class, so the
    // copy's source must publish it — equal by construction to the
    // destination's class through the producer row, but restated per
    // operand so a rebound operand never names a register of another
    // class.
    if consumer.operands.iter().any(|operand| {
        operand.access == RegisterOperandAccess::Use
            && operand.virtual_register == destination
            && operand.class != source_register.class
    }) {
        return Err(CopiedCallOperandError::UnsupportedUse);
    }
    // Between the copy and the call nothing may redefine the source: a
    // rebound operand reads the source at the call's position, which must
    // be the value the copy captured. The producer itself cannot be inside
    // the interval, and the consumer's own definitions are safe — operand
    // uses read pre-definition — so only the open interval is scanned. The
    // destination needs no scan: the producer is by construction its last
    // definition before the call.
    for instruction in &function.blocks[block_index].instructions[producer_index + 1..position] {
        if instruction.operands.iter().any(|operand| {
            operand.access != RegisterOperandAccess::Use
                && operand.virtual_register == copied_source
        }) {
            return Err(CopiedCallOperandError::UnsupportedUse);
        }
    }
    // The declared effect surface, checked against the bound catalog: the
    // retained copy is fully effect-isolated while the call row — the
    // consumer's and the rewritten form's alike — carries the complete
    // activation contract: the `Call` barrier, the
    // `DirectInternalNormalReturnV1` call effect, `DirectRelativeCallV1`
    // control, the retained `MayArchitecturalFaultV1` trap, the row's
    // implicit-unit roster restated, and the target's stack lifecycle on
    // every alternative.
    let producer_declaration = effect_declaration(
        effect_catalog,
        rule.producer(),
        producer_instruction.constraint,
    )
    .ok_or(CopiedCallOperandError::EffectSurfaceMismatch)?;
    let call_declaration = effect_declaration(effect_catalog, consumer_kind, consumer.constraint)
        .ok_or(CopiedCallOperandError::EffectSurfaceMismatch)?;
    if !rule.admits_declarations(producer_declaration, call_declaration, row) {
        return Err(CopiedCallOperandError::EffectSurfaceMismatch);
    }
    // Charge the walk: the whole-function scan locates the consumer; a
    // forward scan of this block locates the producer; the backward scan
    // proves the destination's last definition and the interval scan
    // audits the instructions between them for a source redefinition.
    let block = &function.blocks[block_index];
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
                .checked_add(block.instructions.len())?
                .checked_add(block.instructions.len())
        })
        .ok_or(CopiedCallOperandError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| CopiedCallOperandError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(CopiedCallOperandError::WorkBudgetExceeded);
    }
    Ok(AdmittedPair {
        function,
        block_index,
        position,
        source: copied_source,
        destination,
    })
}

/// The one-instruction proposal shape shared with replay: the consumer
/// keeps its identity, kind, constraint row, operand order, ABI views,
/// implicit surfaces, and provenance; only the `Use` operands that named
/// the copy's destination rebind to the source register.
pub(super) fn rewritten(admitted: &AdmittedPair<'_>) -> SelectedInstruction {
    let mut instruction =
        admitted.function.blocks[admitted.block_index].instructions[admitted.position].clone();
    for operand in &mut instruction.operands {
        if operand.access == RegisterOperandAccess::Use
            && operand.virtual_register == admitted.destination
        {
            operand.virtual_register = admitted.source;
        }
    }
    instruction
}

/// The single catalog declaration for `semantic` bound to `constraint`, or
/// none when the catalog does not declare exactly one such form.
fn effect_declaration(
    catalog: &ValidatedMachineEffectCatalog,
    semantic: MachineSemanticKind,
    constraint: register_model::RegisterConstraintKey,
) -> Option<&MachineEffectDeclaration> {
    let mut matches = catalog.catalog().declarations.iter().filter(|declaration| {
        declaration.semantic == semantic && declaration.constraint == constraint
    });
    let declaration = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(declaration)
}

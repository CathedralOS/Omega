//! Producer admission of a projected-access pair.
//!
//! The descriptor table drives the admission: the consumer instruction's
//! semantic kind selects the declared consumers, the pointer operand's
//! in-block producer completes the pair, and the rule's axes — operand
//! shape, displacement bound, unit flow, effect surface — state every gate
//! the concrete records must satisfy. What no catalog declaration can
//! attest is checked on the function itself: the pointer's last definition
//! before the consumer, the base register's freedom from intervening
//! redefinition, the operand classes against the roster, and the
//! whole-function work all charge into the caller's budget. The replay in
//! `replay` re-derives all of it without the table.

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    MachineEffectDeclaration, MachineSemanticKind, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, ValidatedMachineEffectCatalog,
    VirtualRegisterId,
};

use super::ProjectedAccessError;
use super::pair::{ProjectedAccessRule, access_width, declared_consumer, projected_access_for};
use crate::ValidatedSelectedAnalysis;
use crate::machine_semantic_kind;

/// One admitted projected-access pair: everything `rewrite` needs to
/// rebuild the consumer instruction and everything `replay` needs to
/// re-check it.
pub(super) struct AdmittedPair<'source> {
    pub(super) function: &'source SelectedFunction,
    pub(super) block_index: usize,
    /// The consumer's position in its block's instruction stream.
    pub(super) position: usize,
    /// The producer's base register: the consumer's operand 0 rebinds to
    /// it.
    pub(super) base: VirtualRegisterId,
    /// The consumer's kind carrying the combined displacement.
    pub(super) kind: SelectedInstructionKind,
}

/// The instruction defining `register` at `position`: the last instruction
/// in the block before it carrying a non-`Use` operand on that register.
/// Blocks execute in order, so that definition is the value the consumer
/// observes; edge and entry bindings can only reach it when no body
/// instruction defines the register, in which case there is no in-block
/// `AddressOffset` producer to fold.
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

/// Admit `access` — a body instruction in `function_index`'s function — as
/// a projected-access pair under the descriptor table.
///
/// The order of gates matters for the error vocabulary: the record shape
/// (consumer kind, operand grammar, unit surface) refuses first, then the
/// constraint-row contract, then the producer — the pointer's last
/// definition must be a declared `AddressOffset` — and the base's freedom
/// from intervening redefinition, then the combined-displacement bound,
/// then the catalog declarations' retained-access surface, and last the
/// bounded-work charge.
pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    access: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    effect_catalog: &ValidatedMachineEffectCatalog,
    budget: OptimizationWorkBudget,
) -> Result<AdmittedPair<'source>, ProjectedAccessError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(ProjectedAccessError::SourceMismatch);
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
        return Err(ProjectedAccessError::EffectSurfaceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(ProjectedAccessError::SourceMismatch)?;
    // The consumer is a body instruction: a dereference at a mid-block
    // position. A terminator's carried instruction is not an access form
    // the family declares.
    let (block_index, position, consumer) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .enumerate()
                .find(|(_, instruction)| instruction.id == access)
                .map(|(position, instruction)| (block_index, position, instruction))
        })
        .ok_or(ProjectedAccessError::UnsupportedConsumer)?;
    if !declared_consumer(consumer.kind) {
        return Err(ProjectedAccessError::UnsupportedConsumer);
    }
    let consumer_kind = machine_semantic_kind(consumer.kind);
    let width = access_width(consumer.kind).ok_or(ProjectedAccessError::UnsupportedConsumer)?;
    // The emitted access form is the two-operand shape the declared
    // operand grammar describes: operand 0 the referent pointer `Use`,
    // operand 1 the declared tail access — a `Def` result for the loads,
    // the stored `Use` for the store — and the `RegisterOnly` unit
    // surface: no implicit traffic and no operand bindings the rebuilt
    // record would silently keep. Every rule the consumer kind names must
    // agree on the tail access — a divergent declaration is a catalog
    // defect, not a fold.
    let rule_candidates: Vec<&ProjectedAccessRule> = super::pair::PROJECTED_ACCESS_RULES
        .iter()
        .filter(|rule| rule.consumer() == consumer_kind)
        .collect();
    let [pointer_operand, tail_operand] = consumer.operands.as_slice() else {
        return Err(ProjectedAccessError::UnsupportedConsumer);
    };
    if pointer_operand.operand != 0
        || pointer_operand.access != RegisterOperandAccess::Use
        || tail_operand.operand != 1
        || !rule_candidates
            .iter()
            .all(|rule| tail_operand.access == rule.tail_access())
        || !rule_candidates
            .iter()
            .all(|rule| rule.admits_record(consumer))
    {
        return Err(ProjectedAccessError::UnsupportedConsumer);
    }
    let pointer = pointer_operand.virtual_register;
    let find_register = |register| {
        function
            .virtual_registers
            .iter()
            .find(|entry| entry.id == register)
    };
    let pointer_register = find_register(pointer).ok_or(ProjectedAccessError::UnsupportedUse)?;
    let tail_register =
        find_register(tail_operand.virtual_register).ok_or(ProjectedAccessError::UnsupportedUse)?;
    // The consumer's declared constraint row must publish the same operand
    // shape and classes its operands carry, and each operand's class must
    // equal its roster row's — a different row would change what operand
    // zero means, and a roster class the operand does not declare would
    // leave the rebound register publishing the wrong class. The row's own
    // unit traffic must satisfy the declared `RegisterOnly` surface.
    let row = environment
        .constraint(consumer.constraint)
        .ok_or(ProjectedAccessError::ConstraintMismatch)?;
    if row.operands.len() != 2
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[0].class != pointer_operand.class
        || pointer_operand.class != pointer_register.class
        || row.operands[1].operand != 1
        || !rule_candidates
            .iter()
            .all(|rule| row.operands[1].access == rule.tail_access())
        || row.operands[1].class != tail_operand.class
        || tail_operand.class != tail_register.class
        || !rule_candidates
            .iter()
            .all(|rule| rule.admits_row_units(row))
    {
        return Err(ProjectedAccessError::ConstraintMismatch);
    }
    // The pointer the consumer reads must come from the block's last
    // definition of it before the consumer, and that instruction must be a
    // clean `AddressOffset`: `[use base, def pointer]` under the same
    // `RegisterOnly` surface. A projection that rewrote its own base
    // (`base == pointer`) leaves operand zero reading the projected value,
    // not the base — refusing rather than folding a shifted sum.
    let producer_index = last_definition_before(
        &function.blocks[block_index].instructions,
        position,
        pointer,
    )
    .ok_or(ProjectedAccessError::UnsupportedProducer)?;
    let producer = &function.blocks[block_index].instructions[producer_index];
    let SelectedInstructionKind::AddressOffset {
        byte_offset: producer_offset,
    } = producer.kind
    else {
        return Err(ProjectedAccessError::UnsupportedProducer);
    };
    let rule = projected_access_for(machine_semantic_kind(producer.kind), consumer_kind)
        .ok_or(ProjectedAccessError::UnsupportedProducer)?;
    if !rule.matches_producer(producer.kind) || !rule.admits_record(producer) {
        return Err(ProjectedAccessError::UnsupportedProducer);
    }
    debug_assert_eq!(
        rule.rewritten(),
        consumer_kind,
        "every declared projected-access pair rewrites to the consumer's own semantic"
    );
    let [base_operand, result_operand] = producer.operands.as_slice() else {
        return Err(ProjectedAccessError::UnsupportedProducer);
    };
    if base_operand.operand != 0
        || base_operand.access != RegisterOperandAccess::Use
        || result_operand.operand != 1
        || result_operand.access != RegisterOperandAccess::Def
        || result_operand.virtual_register != pointer
    {
        return Err(ProjectedAccessError::UnsupportedProducer);
    }
    let base = base_operand.virtual_register;
    if base == pointer {
        return Err(ProjectedAccessError::UnsupportedProducer);
    }
    let base_register = find_register(base).ok_or(ProjectedAccessError::UnsupportedUse)?;
    // The rebound operand keeps the consumer operand's declared class, so
    // the base register must publish that class — and the producer's own
    // row must declare the `[use, def]` shape its operands carry at the
    // classes the roster assigns.
    if base_register.class != pointer_operand.class {
        return Err(ProjectedAccessError::UnsupportedUse);
    }
    let producer_row = environment
        .constraint(producer.constraint)
        .ok_or(ProjectedAccessError::ConstraintMismatch)?;
    if producer_row.operands.len() != 2
        || producer_row.operands[0].operand != 0
        || producer_row.operands[0].access != RegisterOperandAccess::Use
        || producer_row.operands[0].class != base_operand.class
        || base_operand.class != base_register.class
        || producer_row.operands[1].operand != 1
        || producer_row.operands[1].access != RegisterOperandAccess::Def
        || producer_row.operands[1].class != result_operand.class
        || result_operand.class != pointer_register.class
        || !rule.admits_row_units(producer_row)
    {
        return Err(ProjectedAccessError::ConstraintMismatch);
    }
    // Between the producer and the consumer nothing may redefine the base:
    // the folded operand reads the base at the consumer's position, which
    // must be the value the producer offset. The producer itself cannot be
    // inside the interval, and the consumer's own definitions are safe —
    // operand uses read pre-definition — so only the open interval is
    // scanned. The pointer needs no scan: the producer is by construction
    // its last definition before the consumer.
    for instruction in &function.blocks[block_index].instructions[producer_index + 1..position] {
        if instruction.operands.iter().any(|operand| {
            operand.access != RegisterOperandAccess::Use && operand.virtual_register == base
        }) {
            return Err(ProjectedAccessError::UnsupportedUse);
        }
    }
    // The declared immediate bound: the consumer's `byte_offset` and the
    // producer's combine under the access-scaled unsigned immediate every
    // target's form shares.
    let consumer_offset = match consumer.kind {
        SelectedInstructionKind::Load8 { byte_offset }
        | SelectedInstructionKind::Load16 { byte_offset }
        | SelectedInstructionKind::Load32 { byte_offset }
        | SelectedInstructionKind::Load64 { byte_offset }
        | SelectedInstructionKind::Store { byte_offset, .. } => byte_offset,
        _ => return Err(ProjectedAccessError::UnsupportedConsumer),
    };
    let combined = consumer_offset
        .checked_add(producer_offset)
        .ok_or(ProjectedAccessError::UnsupportedDisplacement)?;
    if !rule.admits_displacement(combined, width) {
        return Err(ProjectedAccessError::UnsupportedDisplacement);
    }
    let kind = match consumer.kind {
        SelectedInstructionKind::Load8 { .. } => SelectedInstructionKind::Load8 {
            byte_offset: combined,
        },
        SelectedInstructionKind::Load16 { .. } => SelectedInstructionKind::Load16 {
            byte_offset: combined,
        },
        SelectedInstructionKind::Load32 { .. } => SelectedInstructionKind::Load32 {
            byte_offset: combined,
        },
        SelectedInstructionKind::Load64 { .. } => SelectedInstructionKind::Load64 {
            byte_offset: combined,
        },
        SelectedInstructionKind::Store { byte_size, .. } => SelectedInstructionKind::Store {
            byte_offset: combined,
            byte_size,
        },
        _ => return Err(ProjectedAccessError::UnsupportedConsumer),
    };
    // The declared effect surface, checked against the bound catalog: the
    // retained producer is fully effect-isolated while the access row —
    // the consumer's and the rewritten form's alike — carries the
    // dereference's own `ReadPointerV1`/`WritePointerV1` memory and the
    // retained `MayArchitecturalFaultV1` trap, fall-through and
    // stack-unchanged, on every alternative.
    let producer_declaration =
        effect_declaration(effect_catalog, rule.producer(), producer.constraint)
            .ok_or(ProjectedAccessError::EffectSurfaceMismatch)?;
    let access_declaration = effect_declaration(effect_catalog, consumer_kind, consumer.constraint)
        .ok_or(ProjectedAccessError::EffectSurfaceMismatch)?;
    if !rule.admits_declarations(producer_declaration, access_declaration, width) {
        return Err(ProjectedAccessError::EffectSurfaceMismatch);
    }
    // Charge the walk: the whole-function scan locates the consumer; a
    // backward scan of this block locates the pointer's last definition;
    // the interval scan then audits the instructions between it and the
    // consumer for a base redefinition.
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
        .ok_or(ProjectedAccessError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| ProjectedAccessError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(ProjectedAccessError::WorkBudgetExceeded);
    }
    Ok(AdmittedPair {
        function,
        block_index,
        position,
        base,
        kind,
    })
}

/// The one-instruction proposal shape shared with replay: the consumer
/// keeps its identity, constraint row, operand records, implicit surfaces,
/// and provenance; only the kind's displacement and operand zero's
/// register change.
pub(super) fn rewritten(admitted: &AdmittedPair<'_>) -> SelectedInstruction {
    let mut instruction =
        admitted.function.blocks[admitted.block_index].instructions[admitted.position].clone();
    instruction.kind = admitted.kind;
    instruction.operands[0].virtual_register = admitted.base;
    instruction
}

/// The single catalog declaration for `semantic` bound to `constraint`, or
/// none when the catalog does not declare exactly one such form.
fn effect_declaration<'catalog>(
    catalog: &'catalog ValidatedMachineEffectCatalog,
    semantic: MachineSemanticKind,
    constraint: register_model::RegisterConstraintKey,
) -> Option<&'catalog MachineEffectDeclaration> {
    let mut matches = catalog.catalog().declarations.iter().filter(|declaration| {
        declaration.semantic == semantic && declaration.constraint == constraint
    });
    let declaration = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(declaration)
}

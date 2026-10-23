//! Producer derivation of exact literal-fold actions.

use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedBlockId, SelectedFunction, SelectedInstruction, SelectedInstructionKind,
    VirtualRegisterId, VirtualRegisterOrigin, terminator_successors,
};
use semantic_vocabulary::IntegerValue;

use crate::{LiteralFoldAction, PairUnitDefRelation};

use super::constraints::AdmittedPairs;
use super::function_rewrite::rewritten_consumer;
use crate::rewrites::operand_probes::effect_declaration;

/// The sole-use site a nominated literal folds into: the consumer's position
/// in the literal's own block and the operand number its `Use` record
/// carries.
struct SoleUse {
    consumer_index: usize,
    operand: u16,
}

/// Nominate and admit every foldable materialized literal in `function`, in
/// block and instruction order.
///
/// Nomination is a scan of the instruction stream, not a classification
/// lookup: each `MaterializeI64` record — a pressure-recovery victim or an
/// ordinary lowering materialization — is a candidate when its producer
/// record carries the custody the fold consumes and its defined register
/// occurs nowhere else in the function but at one `Use` operand of an
/// instruction later in the same block. Every check the scan cannot
/// discharge is a decline, not an error: a register with a second use or
/// definition, a use in another block or a terminator or a successor
/// transport, and a consumer no enabled pair admits all leave the record in
/// place — the fold proves custody or it does not touch the program.
/// Errors remain for facts internal to the fold's own bookkeeping: an
/// apply-time lookup or identifier remap failing on the stream it just
/// walked is an inconsistency, not a program state.
pub(super) fn derive_actions(
    function: &SelectedFunction,
    rows: &AdmittedPairs<'_>,
) -> Vec<LiteralFoldAction> {
    let mut actions = Vec::new();
    for (block_index, block) in function.blocks.iter().enumerate() {
        for (literal_index, literal) in block.instructions.iter().enumerate() {
            let SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(value),
            } = literal.kind
            else {
                continue;
            };
            let Some(victim) = producer_victim(literal) else {
                continue;
            };
            let Some(literal_u64) = u64::try_from(value).ok() else {
                continue;
            };
            if !producer_record_admits(literal)
                || !register_admits(function, literal, victim, value)
            {
                continue;
            }
            let Some(site) = sole_use(function, block_index, literal_index, literal, victim) else {
                continue;
            };
            let consumer = &block.instructions[site.consumer_index];
            // A consumer rewrites once: when an earlier literal in scan
            // order already claimed it, the later candidate declines.
            if actions
                .iter()
                .any(|action: &LiteralFoldAction| action.consumer_instruction == consumer.id)
            {
                continue;
            }
            if let Some(action) = admit_candidate(
                function,
                block_index,
                block.id,
                literal,
                site.consumer_index,
                consumer,
                site.operand,
                victim,
                literal_u64,
                rows,
            )
            .filter(|action| rewritten_consumer(0, function, action, rows).is_ok())
            {
                actions.push(action);
            }
        }
    }
    actions
}

/// The register a `MaterializeI64` defines, when its operand list is exactly
/// the one undecorated `Def` the kind carries. Any other operand shape is a
/// record the fold cannot reason about and declines.
fn producer_victim(literal: &SelectedInstruction) -> Option<VirtualRegisterId> {
    let [operand] = literal.operands.as_slice() else {
        return None;
    };
    (operand.access == RegisterOperandAccess::Def
        && operand.fixed_view.is_none()
        && operand.tied_to.is_none()
        && !operand.early_clobber)
        .then_some(operand.virtual_register)
}

/// Producer custody the eliminated instruction must carry: no unit traffic
/// at all — removing it would silently drop any implicit use, definition,
/// clobber, or operand binding it declared — and the single-operation
/// provenance shape the recovery classifier required, derived here from the
/// record itself: one operation, one produced value, no edges or
/// obligations, and fuel that settles only at that operation. A
/// materialization failing any clause stays in the stream untouched.
fn producer_record_admits(literal: &SelectedInstruction) -> bool {
    literal.implicit_uses.is_empty()
        && literal.implicit_defs.is_empty()
        && literal.clobbers.is_empty()
        && literal.provenance.operations.len() == 1
        && literal.provenance.values.len() == 1
        && literal.provenance.edges.is_empty()
        && literal.provenance.obligations.is_empty()
        && !literal.provenance.fuel.is_empty()
        && literal.provenance.fuel.iter().all(|fuel| {
            fuel.site
                == optimization_unit::PsiProvenance::Operation(literal.provenance.operations[0])
        })
}

/// Victim custody from the register table: the defined register must be an
/// integer scalar that admits the literal — any width and sign, since the
/// fold grammars name only the bit pattern — with no entry fixed-view pin,
/// and an `InstructionResult` origin naming this literal and the same source
/// value the provenance carries. A register the scan cannot tie back to
/// this materialization is never folded.
fn register_admits(
    function: &SelectedFunction,
    literal: &SelectedInstruction,
    victim: VirtualRegisterId,
    value: u128,
) -> bool {
    let Some(register) = function.virtual_register(victim) else {
        return false;
    };
    let Some(integer) = register.scalar_type.as_integer() else {
        return false;
    };
    !integer.is_address()
        && integer.admits(IntegerValue::Unsigned(value))
        && register.entry_fixed_view.is_none()
        && matches!(
            register.origin,
            VirtualRegisterOrigin::InstructionResult {
                instruction,
                source_value,
            } if instruction == literal.id
                && literal.provenance.values.as_slice() == [source_value]
        )
}

/// The sole-use fact the fold consumes: across the whole function —
/// instruction operands, terminator operands, and every successor binding
/// transport — the victim's only occurrences are the literal's own `Def`
/// and one `Use` at an operand of an instruction later in the same block.
/// A use anywhere else, a second definition, a `UseDef` alias, or a
/// successor-transport reference leaves the register live past the fold and
/// the candidate declines. This is the same occurrence set the classifier's
/// range-fragment checks and the rewrite's own dropped-`Def` custody walk,
/// derived here without consulting either artifact.
fn sole_use(
    function: &SelectedFunction,
    block_index: usize,
    literal_index: usize,
    literal: &SelectedInstruction,
    victim: VirtualRegisterId,
) -> Option<SoleUse> {
    let mut site = None;
    for (candidate_block_index, block) in function.blocks.iter().enumerate() {
        let terminator = selected_instructions::terminator_instruction(&block.terminator);
        for (instruction_index, (instruction, in_block_body)) in block
            .instructions
            .iter()
            .map(|instruction| (instruction, true))
            .chain(std::iter::once((terminator, false)))
            .enumerate()
        {
            for operand in &instruction.operands {
                if operand.virtual_register != victim {
                    continue;
                }
                match operand.access {
                    RegisterOperandAccess::Def | RegisterOperandAccess::UseDef => {
                        // The only `Def` the scan may find is the literal's
                        // own operand; any other write leaves a second
                        // producer behind.
                        if candidate_block_index != block_index
                            || instruction.id != literal.id
                            || operand.access != RegisterOperandAccess::Def
                        {
                            return None;
                        }
                    }
                    RegisterOperandAccess::Use => {
                        if site.is_some() {
                            return None;
                        }
                        site = Some((
                            candidate_block_index,
                            in_block_body,
                            instruction_index,
                            operand.operand,
                        ));
                    }
                }
            }
        }
        let successors = terminator_successors(&block.terminator);
        for successor in successors {
            let carried = successor.structural_bindings.iter().any(|binding| {
                matches!(
                    binding.transport,
                    selected_instructions::SelectedStructuralTransport::WholeValue {
                        argument,
                        ..
                    }
                    | selected_instructions::SelectedStructuralTransport::ScalarReference {
                        base: selected_instructions::SelectedAddressBase::Register(argument),
                        ..
                    }
                    | selected_instructions::SelectedStructuralTransport::Address {
                        base: selected_instructions::SelectedAddressBase::Register(argument),
                        ..
                    } if argument == victim
                )
            }) || successor.structural_case.iter().any(|case| {
                case.payloads
                    .iter()
                    .any(|payload| match &payload.transport {
                        selected_instructions::SelectedCasePayloadTransport::Unused => false,
                        selected_instructions::SelectedCasePayloadTransport::Unmaterialized {
                            parameter,
                        } => *parameter == victim,
                        selected_instructions::SelectedCasePayloadTransport::Registers {
                            argument,
                            parameter,
                        } => *argument == victim || *parameter == victim,
                    })
            }) || successor
                .bindings
                .iter()
                .any(|binding| match &binding.transport {
                    selected_instructions::SelectedValueTransport::Unused => false,
                    selected_instructions::SelectedValueTransport::Registers {
                        argument,
                        parameter,
                    } => *argument == victim || *parameter == victim,
                });
            if carried {
                return None;
            }
        }
    }
    let (site_block, in_block_body, instruction_index, operand) = site?;
    (site_block == block_index && in_block_body && instruction_index > literal_index).then_some(
        SoleUse {
            consumer_index: instruction_index,
            operand,
        },
    )
}

/// Admit one nominated candidate under the enabled pair grammars. Every
/// admission failure — an unadmitted consumer kind, operand position, or
/// literal bound; an operand shape the grammar does not cover; custody the
/// record cannot discharge; an effect surface the bound catalog does not
/// isolate — declines the candidate: the record stays and no fold applies.
#[allow(clippy::too_many_arguments)]
fn admit_candidate(
    function: &SelectedFunction,
    block_index: usize,
    block: SelectedBlockId,
    literal: &SelectedInstruction,
    consumer_index: usize,
    consumer: &SelectedInstruction,
    use_operand: u16,
    victim_register: VirtualRegisterId,
    literal_u64: u64,
    rows: &AdmittedPairs<'_>,
) -> Option<LiteralFoldAction> {
    // The consumer kind, the folded literal's operand position, and the
    // literal's value together select the grammar: a family may admit the
    // same kind under disjoint operand shapes, and disjoint families may
    // admit the same kind at the same position under disjoint literal
    // bounds — the bitwise-and annihilator and identity selections both
    // fold `BitwiseAndI64` at either `Use` position.
    let pair = rows.for_consumer(consumer.kind, use_operand, literal_u64)?;
    let immediate = pair.rule.fold_immediate(literal_u64)?;
    if !pair.rule.matches_producer(literal.kind)
        || literal.operands.len() != 1
        || literal.operands[0].virtual_register != victim_register
        || literal.operands[0].access != RegisterOperandAccess::Def
        // The eliminated instruction's record must carry no unit traffic at
        // all: removing it would silently drop any implicit use, definition,
        // clobber, or operand binding it declared.
        || !pair.rule.unit_effects().admits_producer(literal)
    {
        return None;
    }

    let row = pair.row;
    // The declared grammar fixes the consumer's operand roles — where the
    // folded literal sits, which register input survives, and where the
    // scalar result is — so the producer reads the axes instead of
    // recognising each named shape. Every operand past those roles is the
    // declared tail custody's to discharge.
    let shape = pair.rule.operand_shape();
    let victim_operand = usize::from(shape.victim_operand());
    let survivor_operand = shape.survivor_operand().map(usize::from);
    let result_operand = shape.result_operand(pair.rule.result()).map(usize::from);
    let mut operand_roles = victim_operand + 1;
    for index in survivor_operand.into_iter().chain(result_operand) {
        operand_roles = operand_roles.max(index + 1);
    }
    let Some((grammar, tail)) = consumer.operands.split_at_checked(operand_roles) else {
        return None;
    };
    if !shape.tail.admits(function, tail) {
        return None;
    }
    if grammar[victim_operand].access != RegisterOperandAccess::Use
        || grammar[victim_operand].virtual_register != victim_register
    {
        return None;
    }
    if survivor_operand.is_some_and(|index| grammar[index].access != RegisterOperandAccess::Use) {
        return None;
    }
    let scalar_result = result_operand.map(|index| &grammar[index]);
    if scalar_result.is_some_and(|result| result.access != RegisterOperandAccess::Def) {
        return None;
    }
    // The rewritten row carries exactly the roles the declared axes fix:
    // the surviving `Use` the rewrite binds and the scalar `Def` it writes,
    // each at its own canonical index.
    let (binds_surviving, binds_result) = pair.rule.rewritten_row_roles()?;
    let row_operands = pair.row.operands.as_slice();
    let expected_row_operands = usize::from(binds_surviving) + usize::from(binds_result);
    if row_operands.len() != expected_row_operands
        || row_operands
            .iter()
            .enumerate()
            .any(|(index, operand)| usize::from(operand.operand) != index)
    {
        return None;
    }
    if binds_surviving
        && row_operands[0].class != survivor_operand.map(|index| &grammar[index])?.class
    {
        return None;
    }
    // The operand-swapped grammar rebuilds the row from the consumer
    // record's own unit surface — the reader audit covers the record's
    // definitions — so an implicit use or clobber the row does not carry
    // would silently stop being observed.
    if pair.rule.machine_effects().unit_defs == PairUnitDefRelation::OperandSwapped
        && (consumer.implicit_uses != pair.row.implicit_uses
            || consumer.implicit_defs != pair.row.implicit_defs
            || consumer.clobbers != pair.row.clobbers)
    {
        return None;
    }
    let result = if !binds_result {
        None
    } else {
        let row_def = &row_operands[expected_row_operands - 1];
        let result = scalar_result?;
        if row_def.access != RegisterOperandAccess::Def || row_def.class != result.class {
            return None;
        }
        Some(result.virtual_register)
    };
    // The rewrite rebuilds the consumer's operands from the rewritten
    // constraint row; an operand carrying a unit binding would silently
    // lose it, so the declared unit-effect surface admits only undecorated
    // consumer operands.
    if !pair.rule.unit_effects().admits_consumer(consumer) {
        return None;
    }
    // The obligation-discharged surface also needs the obligation's
    // instruction-level custody: under `DischargedByObligation` the
    // folded literal does not itself make the encoded fault unreachable,
    // so the fold is admitted only when the obligation the consumer kind
    // names is retained in the instruction's recorded proof custody.
    if !pair
        .rule
        .machine_effects()
        .admits_consumer_obligation(consumer)
    {
        return None;
    }
    // The dead-unit-defs surface also needs its record-level half: under
    // `RetiredWhenDead` the rewrite retires every implicit unit the
    // consumer defines, admitted only while no instruction or terminator
    // in the function implicitly uses such a unit — a reader would
    // observe a stale unit once the defining instruction disappears. The
    // whole-function scan is a fact no catalog declaration can attest, so
    // it runs here on the concrete consumer record.
    if !pair
        .rule
        .machine_effects()
        .admits_dead_consumer_defs(consumer, function)
    {
        return None;
    }
    // The operand-swapped unit-defs surface also needs its record-level
    // half: under `OperandSwapped` the rewrite keeps the
    // consumer's implicit definitions but reverses the comparison's
    // operand order — the equality predicate is preserved and every
    // ordering predicate inverts — so the fold is admitted only while
    // every reader each defined unit reaches through the CFG is
    // equality-sensing. The flow audit is a fact no catalog declaration
    // can attest, so it runs here on the concrete consumer record.
    if !pair.rule.machine_effects().admits_swapped_condition_defs(
        consumer,
        function,
        block_index,
        consumer_index,
    ) {
        return None;
    }
    // The pair's declared machine-effect surface must hold in the bound
    // catalog for both instructions the rewrite touches: the eliminated
    // literal must be fully effect-isolated so its removal drops nothing
    // machine-visible, and the consumer must be isolated outside the unit
    // surface the rewrite replaces wholesale. The rewritten declaration was
    // already admitted against the same surface when the pair was bound.
    let producer_declaration =
        effect_declaration(rows.catalog, pair.rule.producer(), literal.constraint)?;
    let consumer_declaration =
        effect_declaration(rows.catalog, pair.rule.consumer(), consumer.constraint)?;
    let effects = pair.rule.machine_effects();
    if !effects.admits_producer(producer_declaration)
        || !effects.admits_consumer(consumer_declaration, pair.declaration)
    {
        return None;
    }
    // The action records the register every `Use` position of the rewritten
    // row binds — the source operand that survives the fold: the non-victim
    // input under the binary grammars, the folded input itself under the
    // `Use`-free unary grammar. The constant-result grammars bind no `Use`
    // position; they record the dropped non-victim input for custody.
    let surviving = grammar
        [usize::from(shape.survivor_operand().unwrap_or(shape.victim_operand()))]
    .virtual_register;

    Some(LiteralFoldAction {
        block,
        literal_instruction: literal.id,
        victim: victim_register,
        consumer_instruction: consumer.id,
        surviving,
        result,
        immediate,
        immediate_constraint: row.key,
    })
}

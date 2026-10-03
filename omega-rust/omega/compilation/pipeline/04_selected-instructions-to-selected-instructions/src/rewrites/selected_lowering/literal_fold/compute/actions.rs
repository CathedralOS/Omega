//! Producer derivation of exact literal-fold actions.

use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedBlockId, SelectedFunction, SelectedInstruction, SelectedInstructionKind,
    SelectedTerminator, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerValue, ScalarType};

use crate::{LiteralFoldAction, PairLiteralPosition, PairOperandShape, PairResultDisposition};

use super::constraints::{AdmittedPairs, effect_declaration};
use super::function_rewrite::rewritten_consumer;

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
    let Some(register) = function
        .virtual_registers
        .iter()
        .find(|register| register.id == victim)
    else {
        return false;
    };
    let ScalarType::Integer(integer) = register.scalar_type else {
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
        let terminator = match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
            | SelectedTerminator::Jump { instruction, .. }
            | SelectedTerminator::Return { instruction, .. }
            | SelectedTerminator::Crash { instruction, .. }
            | SelectedTerminator::HostedExitProcess { instruction, .. } => instruction,
        };
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
        let successors = match &block.terminator {
            SelectedTerminator::Jump { successor, .. } => vec![successor],
            SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } => vec![when_nonzero, when_zero],
            SelectedTerminator::ConditionalBranchU64LessThan {
                when_less,
                when_not_less,
                ..
            }
            | SelectedTerminator::ConditionalBranchI64LessThan {
                when_less,
                when_not_less,
                ..
            } => vec![when_less, when_not_less],
            SelectedTerminator::Return { .. }
            | SelectedTerminator::Crash { .. }
            | SelectedTerminator::HostedExitProcess { .. } => Vec::new(),
        };
        for successor in successors {
            let carried = successor.structural_bindings.iter().any(|binding| {
                matches!(
                    binding.transport,
                    selected_instructions::SelectedStructuralTransport::WholeValue {
                        argument,
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
    let result = match (
        pair.rule.operand_shape(),
        pair.rule.result(),
        consumer.operands.as_slice(),
    ) {
        (
            PairOperandShape::BINARY_RIGHT_LITERAL,
            PairResultDisposition::ScalarRegister,
            [left, right, result],
        ) => {
            if left.access != RegisterOperandAccess::Use
                || right.access != RegisterOperandAccess::Use
                || right.virtual_register != victim_register
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 2
                || left.class != row.operands[0].class
                || result.class != row.operands[1].class
            {
                return None;
            }
            Some(result.virtual_register)
        }
        // A binary right-literal consumer whose operand list continues past
        // its `Def` result with scratch outputs: `[left, victim, result,
        // scratch...]` folds the operand-1 `Use`, binds the operand-0
        // survivor into the rewritten row, and drops every `Def` operand
        // past the result — the bound scratch a clamped saturating
        // realization computes its saturation bound through — admitted
        // only when each dropped register occurs nowhere else in the
        // function. A dropped `Def` another instruction read or defined
        // would silently leave a use of a register the rewrite stopped
        // defining.
        (
            PairOperandShape::BINARY_RIGHT_LITERAL_SCRATCH_DEFS,
            PairResultDisposition::ScalarRegister,
            [left, right, result, scratch @ ..],
        ) => {
            if left.access != RegisterOperandAccess::Use
                || right.access != RegisterOperandAccess::Use
                || right.virtual_register != victim_register
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 2
                || left.class != row.operands[0].class
                || result.class != row.operands[1].class
                || !scratch.iter().all(|operand| {
                    operand.access == RegisterOperandAccess::Def
                        && dropped_def_is_dead(function, operand.virtual_register)
                })
            {
                return None;
            }
            Some(result.virtual_register)
        }
        // A binary right-literal consumer whose operand list continues past
        // its `Def` result: `[left, victim, result, aux...]` folds the
        // operand-1 `Use` and drops every trailing `Use` operand, which the
        // declared grammar admits only when each dropped register is defined
        // in this function solely by zero materializations — the zeroed
        // high-half input an x86-64 `div` realization reads, left dead by a
        // divide by one. A dropped operand defined any other way would
        // silently discard a value the consumer observed.
        (
            PairOperandShape::BINARY_RIGHT_LITERAL_AUXILIARY_USES,
            PairResultDisposition::ScalarRegister,
            [left, right, result, auxiliary @ ..],
        ) => {
            if left.access != RegisterOperandAccess::Use
                || right.access != RegisterOperandAccess::Use
                || right.virtual_register != victim_register
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 2
                || left.class != row.operands[0].class
                || result.class != row.operands[1].class
                || !auxiliary.iter().all(|operand| {
                    operand.access == RegisterOperandAccess::Use
                        && auxiliary_zero_defined(function, operand.virtual_register)
                })
            {
                return None;
            }
            Some(result.virtual_register)
        }
        // A binary right-literal consumer whose operand list continues past
        // its `Def` result with a mixed drop tail: `[left, victim, result,
        // tail...]` folds the operand-1 `Use`, binds the operand-0 survivor
        // into the rewritten row, and drops every operand past the result
        // under the custody its own access declares — a `Use` drops only
        // when its register is defined in this function solely by zero
        // materializations, the zeroed high-half dividend an x86-64
        // `div`/`idiv` realization reads, and a `Def` drops only when its
        // register occurs nowhere else in the function, the bound scratch
        // an aarch64 clamped signed-divide realization writes. A `UseDef`
        // tail operand names neither custody — dropping it would silently
        // discard a read — and rejects.
        (
            PairOperandShape::BINARY_RIGHT_LITERAL_AUXILIARY_USES_OR_SCRATCH_DEFS,
            PairResultDisposition::ScalarRegister,
            [left, right, result, tail @ ..],
        ) => {
            if left.access != RegisterOperandAccess::Use
                || right.access != RegisterOperandAccess::Use
                || right.virtual_register != victim_register
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 2
                || left.class != row.operands[0].class
                || result.class != row.operands[1].class
                || !tail.iter().all(|operand| match operand.access {
                    RegisterOperandAccess::Use => {
                        auxiliary_zero_defined(function, operand.virtual_register)
                    }
                    RegisterOperandAccess::Def => {
                        dropped_def_is_dead(function, operand.virtual_register)
                    }
                    _ => false,
                })
            {
                return None;
            }
            Some(result.virtual_register)
        }
        // A binary right-literal consumer whose folded result is a constant
        // of the literal alone: `[left, victim, result, scratch...]` folds
        // the operand-1 `Use`, drops the operand-0 `Use` — the constant
        // result never reads it — and drops every `Def` operand past the
        // result, which the declared grammar admits only when each dropped
        // register occurs nowhere else in the function — the dead quotient
        // scratch an x86-64 `idiv` realization writes. A dropped `Def`
        // another instruction read or defined would silently leave a use
        // of a register the rewrite stopped defining.
        (
            PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT,
            PairResultDisposition::ScalarRegister,
            [left, right, result, scratch @ ..],
        ) => {
            if left.access != RegisterOperandAccess::Use
                || right.access != RegisterOperandAccess::Use
                || right.virtual_register != victim_register
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 1
                || row.operands[0].access != RegisterOperandAccess::Def
                || result.class != row.operands[0].class
                || !scratch.iter().all(|operand| {
                    operand.access == RegisterOperandAccess::Def
                        && dropped_def_is_dead(function, operand.virtual_register)
                })
            {
                return None;
            }
            Some(result.virtual_register)
        }
        // A binary left-literal consumer whose folded result is a constant
        // of the literal alone: `[victim, right, result, scratch...]`
        // folds the operand-0 `Use`, drops the operand-1 `Use` — the
        // constant result never reads it — and drops every `Def` operand
        // past the result under the same occurrence-free custody the
        // right grammar requires. Declaring this shape attests the
        // operand-0 literal alone fixes the result — `0 & x` is zero for
        // every `x` — so no commutation of the surviving `Use` is
        // implied.
        (
            PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT,
            PairResultDisposition::ScalarRegister,
            [victim, right, result, scratch @ ..],
        ) => {
            if victim.access != RegisterOperandAccess::Use
                || victim.virtual_register != victim_register
                || right.access != RegisterOperandAccess::Use
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 1
                || row.operands[0].access != RegisterOperandAccess::Def
                || result.class != row.operands[0].class
                || !scratch.iter().all(|operand| {
                    operand.access == RegisterOperandAccess::Def
                        && dropped_def_is_dead(function, operand.virtual_register)
                })
            {
                return None;
            }
            Some(result.virtual_register)
        }
        // A binary left-literal consumer whose folded result is a constant
        // of the literal alone and whose operand list continues past its
        // `Def` result with auxiliary `Use` operands:
        // `[victim, right, result, aux...]` folds the operand-0 `Use`,
        // drops the operand-1 `Use`, and drops every trailing `Use` under
        // the same provenance custody the right auxiliary grammar
        // requires — each dropped register defined in this function only
        // by `MaterializeI64` instructions producing `Unsigned(0)`. A
        // literal of zero at operand 0 is only the low dividend half an
        // x86-64 `div` reads; a dropped auxiliary `Use` carrying anything
        // but a proven zero would silently discard the dividend's upper
        // half the folded form stopped observing.
        (
            PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES,
            PairResultDisposition::ScalarRegister,
            [victim, right, result, auxiliary @ ..],
        ) => {
            if victim.access != RegisterOperandAccess::Use
                || victim.virtual_register != victim_register
                || right.access != RegisterOperandAccess::Use
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 1
                || row.operands[0].access != RegisterOperandAccess::Def
                || result.class != row.operands[0].class
                || !auxiliary.iter().all(|operand| {
                    operand.access == RegisterOperandAccess::Use
                        && auxiliary_zero_defined(function, operand.virtual_register)
                })
            {
                return None;
            }
            Some(result.virtual_register)
        }
        // A binary left-literal consumer whose folded result is a constant
        // of the literal alone and whose operand list continues past its
        // `Def` result with a mixed drop tail: `[victim, right, result,
        // tail...]` folds the operand-0 `Use`, drops the operand-1 `Use`
        // — the constant result never reads it — and drops every operand
        // past the result under the custody its own access declares — a
        // `Use` drops only when its register is defined in this function
        // solely by zero materializations, the zeroed high-half dividend
        // an x86-64 `div`/`idiv` realization reads, and a `Def` drops only
        // when its register occurs nowhere else in the function, the
        // bound scratch an aarch64 clamped signed-divide realization
        // writes. A `UseDef` tail operand names neither custody —
        // dropping it would silently discard a read — and rejects.
        (
            PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES_OR_SCRATCH_DEFS,
            PairResultDisposition::ScalarRegister,
            [victim, right, result, tail @ ..],
        ) => {
            if victim.access != RegisterOperandAccess::Use
                || victim.virtual_register != victim_register
                || right.access != RegisterOperandAccess::Use
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 1
                || row.operands[0].access != RegisterOperandAccess::Def
                || result.class != row.operands[0].class
                || !tail.iter().all(|operand| match operand.access {
                    RegisterOperandAccess::Use => {
                        auxiliary_zero_defined(function, operand.virtual_register)
                    }
                    RegisterOperandAccess::Def => {
                        dropped_def_is_dead(function, operand.virtual_register)
                    }
                    _ => false,
                })
            {
                return None;
            }
            Some(result.virtual_register)
        }
        // Commutative binary consumers also admit the literal as the left
        // operand: `[victim, right, result]` folds the operand-0 `Use` and
        // binds the operand-1 survivor into the rewritten row.
        (
            PairOperandShape::BINARY_LEFT_LITERAL,
            PairResultDisposition::ScalarRegister,
            [victim, right, result],
        ) => {
            if victim.access != RegisterOperandAccess::Use
                || victim.virtual_register != victim_register
                || right.access != RegisterOperandAccess::Use
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 2
                || right.class != row.operands[0].class
                || result.class != row.operands[1].class
            {
                return None;
            }
            Some(result.virtual_register)
        }
        // A binary left-literal consumer whose operand list continues past
        // its `Def` result with scratch outputs: `[victim, right, result,
        // scratch...]` folds the operand-0 `Use`, binds the operand-1
        // survivor into the rewritten row, and drops every `Def` operand
        // past the result under the same occurrence-free custody the right
        // scratch-defs grammar requires.
        (
            PairOperandShape::BINARY_LEFT_LITERAL_SCRATCH_DEFS,
            PairResultDisposition::ScalarRegister,
            [victim, right, result, scratch @ ..],
        ) => {
            if victim.access != RegisterOperandAccess::Use
                || victim.virtual_register != victim_register
                || right.access != RegisterOperandAccess::Use
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 2
                || right.class != row.operands[0].class
                || result.class != row.operands[1].class
                || !scratch.iter().all(|operand| {
                    operand.access == RegisterOperandAccess::Def
                        && dropped_def_is_dead(function, operand.virtual_register)
                })
            {
                return None;
            }
            Some(result.virtual_register)
        }
        // Flag-defining consumers carry `[left, right]` uses and no `Def`;
        // their result is the rewritten row's implicit unit definitions.
        (
            PairOperandShape::BINARY_RIGHT_LITERAL,
            PairResultDisposition::ImplicitUnits,
            [left, right],
        ) => {
            if left.access != RegisterOperandAccess::Use
                || right.access != RegisterOperandAccess::Use
                || right.virtual_register != victim_register
                || row.operands.len() != 1
                || left.class != row.operands[0].class
            {
                return None;
            }
            None
        }
        // The operand-swapped flag-defining grammar:
        // `[victim, subtrahend]` folds the operand-0 `Use` — the
        // compare's literal minuend — and binds the operand-1 subtrahend
        // `Use` into the rewritten row's sole `Use` position, computing
        // `subtrahend - literal` for `literal - subtrahend`. The record
        // must publish exactly the implicit unit surface the rewritten
        // row carries: the reader audit covers the record's definitions,
        // so those are the units the rewrite must publish, and an
        // implicit use or clobber the row does not carry would silently
        // stop being observed.
        (
            PairOperandShape::BINARY_LEFT_LITERAL_OPERAND_SWAP,
            PairResultDisposition::ImplicitUnits,
            [victim, subtrahend],
        ) => {
            if victim.access != RegisterOperandAccess::Use
                || victim.virtual_register != victim_register
                || subtrahend.access != RegisterOperandAccess::Use
                || row.operands.len() != 1
                || row.operands[0].access != RegisterOperandAccess::Use
                || subtrahend.class != row.operands[0].class
                || consumer.implicit_uses != row.implicit_uses
                || consumer.implicit_defs != row.implicit_defs
                || consumer.clobbers != row.clobbers
            {
                return None;
            }
            None
        }
        // Unary consumers carry `[input, result]`; the folded literal is the
        // sole `Use` operand and the rewritten row carries only its `Def`.
        (
            PairOperandShape::UNARY_LITERAL,
            PairResultDisposition::ScalarRegister,
            [input, result],
        ) => {
            if input.access != RegisterOperandAccess::Use
                || input.virtual_register != victim_register
                || result.access != RegisterOperandAccess::Def
                || row.operands.len() != 1
                || row.operands[0].access != RegisterOperandAccess::Def
                || result.class != row.operands[0].class
            {
                return None;
            }
            Some(result.virtual_register)
        }
        _ => {
            return None;
        }
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
    // row binds — the source operand that survives the fold. A right-literal
    // grammar — including the auxiliary-`Use` divide grammar and the
    // scratch-defs tail — leaves operand 0, a left-literal grammar leaves
    // operand 1, and the `Use`-free unary fold records its folded input. The
    // constant-result grammars bind no `Use` position; they record the
    // dropped non-victim `Use` for custody — the operand-0 dividend under
    // the right grammar, the operand-1 `Use` under the left annihilator and
    // auxiliary-`Use` grammars.
    let surviving = match pair.rule.operand_shape().position {
        PairLiteralPosition::LeftOperand => consumer.operands[1].virtual_register,
        PairLiteralPosition::RightOperand | PairLiteralPosition::SoleOperand => {
            consumer.operands[0].virtual_register
        }
    };

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

/// Whether `register` is defined in `function` only by `MaterializeI64`
/// instructions producing `Unsigned(0)` — the provenance the
/// auxiliary-`Use` operand grammar requires of every operand it drops. A
/// register with no definition, or any definition that is not a zero
/// materialization, fails: the dropped operand would carry a value the
/// folded form silently stopped observing.
fn auxiliary_zero_defined(
    function: &SelectedFunction,
    register: selected_instructions::VirtualRegisterId,
) -> bool {
    let mut definitions = function
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .filter(|instruction| {
            instruction.operands.iter().any(|operand| {
                operand.access == RegisterOperandAccess::Def && operand.virtual_register == register
            })
        });
    let mut saw_definition = false;
    definitions.all(|instruction| {
        saw_definition = true;
        matches!(
            instruction.kind,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(0)
            }
        )
    }) && saw_definition
}

/// Whether `register`'s only occurrence in `function` is one `Def` operand —
/// the custody the constant-result grammar requires of every scratch `Def`
/// it drops. The operand itself is that one occurrence: any other operand
/// position, terminator operand, or successor transport naming the register
/// would leave the fold removing a definition that a surviving read or a
/// second definition still observes. Uses are counted across instruction
/// and terminator operand lists and every successor binding transport, the
/// same sites the rewrite's densification walks.
fn dropped_def_is_dead(
    function: &SelectedFunction,
    register: selected_instructions::VirtualRegisterId,
) -> bool {
    let mut occurrences = 0_usize;
    for block in &function.blocks {
        for instruction in block.instructions.iter().chain(match &block.terminator {
            selected_instructions::SelectedTerminator::ConditionalBranch {
                instruction, ..
            }
            | selected_instructions::SelectedTerminator::ConditionalBranchU64LessThan {
                instruction,
                ..
            }
            | selected_instructions::SelectedTerminator::ConditionalBranchI64LessThan {
                instruction,
                ..
            }
            | selected_instructions::SelectedTerminator::Jump { instruction, .. }
            | selected_instructions::SelectedTerminator::Return { instruction, .. }
            | selected_instructions::SelectedTerminator::Crash { instruction, .. }
            | selected_instructions::SelectedTerminator::HostedExitProcess {
                instruction, ..
            } => std::iter::once(instruction),
        }) {
            occurrences += instruction
                .operands
                .iter()
                .filter(|operand| operand.virtual_register == register)
                .count();
        }
        let successors = match &block.terminator {
            selected_instructions::SelectedTerminator::Jump { successor, .. } => {
                vec![successor]
            }
            selected_instructions::SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } => vec![when_nonzero, when_zero],
            selected_instructions::SelectedTerminator::ConditionalBranchU64LessThan {
                when_less,
                when_not_less,
                ..
            }
            | selected_instructions::SelectedTerminator::ConditionalBranchI64LessThan {
                when_less,
                when_not_less,
                ..
            } => vec![when_less, when_not_less],
            selected_instructions::SelectedTerminator::Return { .. }
            | selected_instructions::SelectedTerminator::Crash { .. }
            | selected_instructions::SelectedTerminator::HostedExitProcess { .. } => Vec::new(),
        };
        for successor in successors {
            occurrences += successor
                .structural_bindings
                .iter()
                .filter(|binding| {
                    matches!(
                        binding.transport,
                        selected_instructions::SelectedStructuralTransport::WholeValue {
                            argument,
                            ..
                        }
                        | selected_instructions::SelectedStructuralTransport::Address {
                            base: selected_instructions::SelectedAddressBase::Register(argument),
                            ..
                        } if argument == register
                    )
                })
                .count();
            if let Some(case) = &successor.structural_case {
                occurrences += case
                    .payloads
                    .iter()
                    .filter(|payload| match &payload.transport {
                        selected_instructions::SelectedCasePayloadTransport::Unused => false,
                        selected_instructions::SelectedCasePayloadTransport::Unmaterialized {
                            parameter,
                        } => *parameter == register,
                        selected_instructions::SelectedCasePayloadTransport::Registers {
                            argument,
                            parameter,
                        } => *argument == register || *parameter == register,
                    })
                    .count();
            }
            occurrences += successor
                .bindings
                .iter()
                .filter(|binding| match &binding.transport {
                    selected_instructions::SelectedValueTransport::Unused => false,
                    selected_instructions::SelectedValueTransport::Registers {
                        argument,
                        parameter,
                    } => *argument == register || *parameter == register,
                })
                .count();
        }
    }
    occurrences == 1
}

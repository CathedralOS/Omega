pub(in crate::checks::ranges) mod aliases;
mod transitions;

use self::transitions::check_transition_target;
use super::arrays::fixed_array_type_length;
use super::facts::RangeFacts;
use super::indexes::{IndexVerdicts, check_expression};
use super::statement_transfer::{StatementTransferSink, transfer_statement_facts};
use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableAssignment, TransitionTargetHandle};

pub(super) fn check_statement<'program>(
    program: &'program dyn ::validation::CheckingInput,
    machine: &'program Machine,
    state: &'program State,
    call_frames: Option<&validation::CallFrameResolver<'program>>,
    facts: &mut RangeFacts<'_>,
    statement: &'program StatementNode,
    diagnostics: &mut Vec<Diagnostic>,
    verdicts: &mut IndexVerdicts,
) {
    /// The checking pass's sink: every read expression emits index
    /// diagnostics and transition targets are checked under their
    /// edge-narrowed facts. The captured-extent decision stays deferred so
    /// its diagnostic still lands after the value's checks, matching the
    /// original statement order.
    struct CheckSink<'a, 'program> {
        program: &'program dyn ::validation::CheckingInput,
        machine: &'program Machine,
        state: &'program State,
        call_frames: Option<&'a validation::CallFrameResolver<'program>>,
        diagnostics: &'a mut Vec<Diagnostic>,
        verdicts: &'a mut IndexVerdicts,
        extent_failed: bool,
    }
    impl<'a, 'program> StatementTransferSink<'program> for CheckSink<'a, 'program> {
        fn visit_expression(&mut self, facts: &mut RangeFacts<'_>, expression: ExpressionHandle) {
            check_expression(
                self.program,
                self.machine,
                self.state,
                self.call_frames,
                facts,
                expression,
                self.diagnostics,
                self.verdicts,
            );
        }
        fn check_assignment_extent_window(
            &mut self,
            facts: &mut RangeFacts<'_>,
            assignment: &'program TableAssignment,
        ) {
            self.extent_failed |= !super::assignment_lengths::value_preserves_indexed_extent(
                self.program,
                self.machine,
                self.state,
                self.call_frames,
                facts,
                assignment.target,
                assignment.value,
            );
        }
        fn visit_transition_target(
            &mut self,
            facts: &mut RangeFacts<'_>,
            target: TransitionTargetHandle,
        ) {
            check_transition_target(
                self.program,
                self.machine,
                self.state,
                self.call_frames,
                facts,
                target,
                self.diagnostics,
                self.verdicts,
            );
        }
    }
    let mut sink = CheckSink {
        program,
        machine,
        state,
        call_frames,
        diagnostics,
        verdicts,
        extent_failed: false,
    };
    transfer_statement_facts(
        program,
        machine,
        state,
        call_frames,
        facts,
        statement,
        &mut sink,
    );
    if sink.extent_failed {
        sink.diagnostics.push(Diagnostic::error(
            "cannot prove index remains within the captured byte collection's live length across RHS mutation",
        ));
    }
}

/// The extent evidence a freshly bound local REFERENCE inherits from its
/// referent, resolved by `bound_reference_referent_extent`.
pub(super) struct ReferentExtent {
    /// The referent's pinned extent: its declared fixed-array length, or a
    /// recorded exact live length on the referent's own label.
    pub(super) exact: Option<usize>,
    /// The referent's proven live-length floor, when one is recorded.
    /// Variable-fill carriers (`[u8; N] in Domain`) hold their live length in
    /// this lane — their capacity says nothing about the current extent.
    pub(super) minimum: Option<i64>,
}

/// The extent a freshly bound local reference inherits from its single exact
/// referent, or `None` when the binding has no uniquely resolved referent.
///
/// A returned `&mut [T]` owns no storage, and its slice type erases the
/// receiver's extent: `let view = borrow_rows(level)` lends the element
/// storage the callee selected (`level.rooms`), so `view` indexes against
/// THAT extent. The write-origins prefix already proves the referent — the
/// same recovery `flow::transfers`' `bound_reference_referent_place` performs
/// to re-anchor element evidence onto the binding — and
/// `canonical_place_type_reference` projects the referent's declared type for
/// its fixed length. When the referent is a variable-fill carrier whose live
/// length is not its type, the recorded `exact_length`/`minimum_length`
/// facts keyed on the referent's own path supply the extent instead.
///
/// An ambiguous or inexact (runtime-index-truncated) origin stays `None`: a
/// truncated place names a container OF the referent, whose extent is not the
/// bound slice's length, and multiple referents cannot pin one extent.
pub(super) fn bound_reference_referent_extent(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    facts: &RangeFacts<'_>,
    local_symbol: symbols::SymbolHandle,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<ReferentExtent> {
    if !local_symbol.is_valid() {
        return None;
    }
    // Only a reference binding HAS a referent. Peel declarative constraints
    // the same way `bound_reference_referent_place` does; any other declared
    // form means the local's extent must come from its own value evidence.
    let peeled = ::validation::peel_declared_constraints(program, type_reference);
    if !matches!(
        program.type_reference_table.type_reference(peeled),
        typed_trees::types::TypeReferenceNode::Reference { .. }
    ) {
        return None;
    }
    // The binding's own alias is recorded once the NEXT statement begins, so
    // the prefix observed there is the one carrying the new referent.
    let before = program
        .statement_table
        .statements(state.statement_nodes)
        .get(facts.statement_index + 1)?;
    let origin =
        crate::flow::sole_local_referent_origin(call_frames?, machine, before, local_symbol)?;
    let source = origin.source.as_ref()?;
    let (place, precision) =
        crate::flow::origin_place(program, state, facts.statement_index, &origin)?;
    if precision != crate::flow::OriginPrecision::Exact {
        return None;
    }
    let exact = validation::canonical_place_type_reference(
        program,
        state.symbol,
        facts.statement_index,
        &place,
    )
    .and_then(|reference| fixed_array_type_length(program, reference))
    .or_else(|| {
        facts
            .exact_length(&source.path)
            .and_then(|length| usize::try_from(length).ok())
    });
    let minimum = facts.minimum_length(&source.path);
    Some(ReferentExtent { exact, minimum })
}

/// The declared type of the local an assignment rebinds, from its unique
/// earlier `let` in this state — the same declared-type recovery
/// `bound_reference_referent_place` performs for `StatementNode::Assignment`.
pub(super) fn assigned_local_declared_type(
    program: &dyn ::validation::CheckingInput,
    state: &State,
    statement_index: usize,
    local_symbol: symbols::SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    program
        .state_local_declaration(state, local_symbol)
        .filter(|&(ordinal, _)| ordinal < statement_index)
        .map(|(_, local)| local.type_reference)
}

pub(super) fn expression_member_name(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<(symbols::SymbolHandle, Option<&str>)> {
    let member = program.expression_table.member_node(expression)?;
    Some((member.member_symbol, Some(member.member.as_str())))
}

/// The exclusive upper bound a member store `target = source + c` or
/// `target = source - c` gives its target: `target < source_bound + offset`,
/// from `source`'s bound before the store invalidates the target, so
/// `count = count - 1` takes its own former bound one lower. It proves the
/// derived-index pattern `arr[i + 1]` -- a `jp = self.i + 1` field then
/// `arr[self.jp]` inside a loop where `self.i` is bounded by the loop guard
/// (sorts, sliding windows, reversals) -- and the pop pattern
/// `count = count - 1; items[count]`.
///
/// An addition is sound in every domain: Exact makes overflow a proof
/// obligation, Trapping never completes it, and a wrapped or saturated sum is
/// no larger than the exact one. A subtraction needs the exact difference on
/// normal completion, so only Exact and Trapping stores carry it: a wrapped
/// `0 - 1` is the maximum and a saturated one stays at the old bound.
pub(super) fn offset_index_bound(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    target: ExpressionHandle,
    value: ExpressionHandle,
) -> Option<(String, i64)> {
    let (source_name, offset) = field_plus_constant(program, value)?;
    if offset < 0 {
        let domain = program.arithmetic_domain_for_type_reference(
            super::types::expression_type_reference(program, machine, state, target)?,
        );
        if !matches!(domain, ArithmeticDomain::Exact | ArithmeticDomain::Trapping) {
            return None;
        }
    }
    let source_bound = facts.proven_index_upper_bound(&source_name)?;
    let new_bound = source_bound.checked_add(offset)?;
    Some((program.expression_table.display_name(target), new_bound))
}

/// Recognize `field + c` (either operand order) or `field - c` for a positive
/// constant `c`, returning the field's display name and the signed offset.
fn field_plus_constant(
    program: &dyn ::validation::CheckingInput,
    value: ExpressionHandle,
) -> Option<(String, i64)> {
    let Some(binary) = program.expression_table.binary_node(value) else {
        return None;
    };
    let positive_constant = |side: ExpressionHandle| match program.expression_table.expression(side)
    {
        ExpressionNode::Integer(constant) => constant.value_i64().filter(|constant| *constant > 0),
        _ => None,
    };
    match binary.operator {
        BinaryOperator::Add => [(binary.left, binary.right), (binary.right, binary.left)]
            .into_iter()
            .find_map(|(field_side, constant_side)| {
                Some((
                    program.expression_table.display_name(field_side),
                    positive_constant(constant_side)?,
                ))
            }),
        BinaryOperator::Subtract => Some((
            program.expression_table.display_name(binary.left),
            positive_constant(binary.right)?.checked_neg()?,
        )),
        _ => None,
    }
}

/// Resolve a call statement's BOUNDARY-TRAIT callee signature from the typed
/// trees (the bound requirement's applied contract), then seed `ensures <param> <OP>
/// <literal>` conjuncts as index-upper-bound facts on the matching `&mut`
/// argument places. Prior bounds for every `&mut`-written place are
/// forgotten regardless, ensures or not.
pub(super) fn seed_boundary_call_ensures_facts<'program>(
    program: &'program dyn ::validation::CheckingInput,
    machine: &'program Machine,
    state: &State,
    call: &'program typed_trees::statement::TableCall,
    call_frames: Option<&validation::CallFrameResolver<'program>>,
    facts: &mut RangeFacts<'_>,
) {
    // Both callers apply the complete write frame before reaching this
    // postcondition publisher. Borrow syntax alone is not a write footprint.
    // The bound requirement retains the complete applied contract. Receiver
    // spelling or a field declaration cannot substitute another telescope.
    let Some(signature) = typed_trees::boundary::called_boundary_signature(program, call) else {
        return;
    };
    let Some(origins) = call_frames.and_then(|frames| frames.call_write_origins(machine, call))
    else {
        return;
    };
    for conjunct in validation::ensures_contract_expressions(program, signature) {
        seed_ensures_bound_conjunct(program, state, signature, &origins, conjunct, facts);
    }
}

fn seed_ensures_bound_conjunct(
    program: &dyn ::validation::CheckingInput,
    state: &State,
    signature: &typed_trees::signature::StateSignature,
    origins: &validation::CallWriteOrigins,
    conjunct: ExpressionHandle,
    facts: &mut RangeFacts<'_>,
) {
    use typed_trees::expression::BinaryOperator;
    let mut comparisons = Vec::new();
    validation::collect_parameter_bound_comparisons(
        program,
        state,
        signature,
        origins,
        conjunct,
        &mut comparisons,
    );
    for (place, operator, literal) in comparisons {
        // `param <= K` / `param < K`, param on the left (the ensures house
        // spelling); the EXCLUSIVE bound feeds the index prover.
        let exclusive = match operator {
            BinaryOperator::LessOrEqual => 1,
            BinaryOperator::Less => 0,
            _ => continue,
        };
        let Some(bound) = literal.checked_add(exclusive) else {
            continue;
        };
        facts.prove_index_upper_bound(place, bound);
    }
}

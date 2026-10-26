use super::{EXPRESSION_WALK_DEPTH_BOUND, ExpressionHandle, ExpressionNode, Machine, State};
use crate::checks::ranges::facts::RangeCallContext;
use validation::CallSite;
use validation::CanonicalPlace;
use validation::canonical_place_from_expression_in_state;

use checked_trees::CheckedOperatorFacts;
mod atomics;
mod members;
mod operands;
mod root_currency;

use atomics::static_application_carries_no_caller_storage;
use members::has_builtin_index_meaning;
pub(super) use root_currency::root_is_current;

/// The range-read walk context: the program, the machine/state pair under
/// evaluation, the statement occurrence, its checked call and operator
/// custody, the read set the walk fills, and the bound index. Every recursive
/// read step used to restate this tuple (up to eleven arguments) at each call
/// site; the walk now carries it once, and each step names only the node it
/// descends into and the depth.
pub(super) struct ReadWalk<'a, 'b, 'c> {
    program: &'a dyn ::validation::CheckingInput,
    machine: &'a Machine,
    state: &'a State,
    statement_index: usize,
    calls: Option<&'c RangeCallContext<'c>>,
    operators: Option<&'c CheckedOperatorFacts>,
    reads: &'c mut Vec<CanonicalPlace>,
    bound_lookup: &'b validation::ImmutableBoundLookup<'b>,
}

impl<'a, 'b, 'c> ReadWalk<'a, 'b, 'c> {
    pub(super) fn new(
        program: &'a dyn ::validation::CheckingInput,
        machine: &'a Machine,
        state: &'a State,
        statement_index: usize,
        calls: Option<&'c RangeCallContext<'c>>,
        operators: Option<&'c CheckedOperatorFacts>,
        reads: &'c mut Vec<CanonicalPlace>,
        bound_lookup: &'b validation::ImmutableBoundLookup<'b>,
    ) -> Self {
        Self {
            program,
            machine,
            state,
            statement_index,
            calls,
            operators,
            reads,
            bound_lookup,
        }
    }

    /// Whether a read's root is current in the exact authored state prefix —
    /// the shared form of the prefix scan `capture_place_read` and the
    /// operand capture both consult.
    pub(super) fn root_is_current(&self, root: facts::PlaceRoot) -> bool {
        root_currency::root_is_current(
            self.program,
            self.machine,
            self.state,
            self.statement_index,
            root,
        )
    }
}

impl ReadWalk<'_, '_, '_> {
    pub(super) fn collect_reads(&mut self, expression: ExpressionHandle, depth: usize) -> bool {
        let program = self.program;
        let machine = self.machine;
        let state = self.state;
        let statement_index = self.statement_index;
        // Depth bound shared with the captures.rs walks and pinned by
        // tests/depth.rs.
        if depth >= EXPRESSION_WALK_DEPTH_BOUND
            || !program.expression_table.expression_is_valid(expression)
        {
            return false;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::SavedSubject(_) => false,
            // Literal leaves read no caller storage: their bytes are immutable
            // compile-time content, not a place a write could mutate.
            ExpressionNode::Integer(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => true,
            ExpressionNode::Binary(binary) => {
                self.collect_reads(binary.left, depth + 1)
                    && self.collect_reads(binary.right, depth + 1)
            }
            ExpressionNode::Unary(unary) => self.collect_reads(unary.operand, depth + 1),
            ExpressionNode::Cast(cast) => self.collect_reads(cast.value, depth + 1),
            // A borrow's value is a reference into its target place, so its
            // footprint is exactly the target's reads. For a call operand this is
            // also the storage the callee was handed; the checked access rows
            // record the same place once more and dedup absorbs it.
            ExpressionNode::Borrow(inner) => self.collect_reads(inner.target, depth + 1),
            // A call's value is not a pure function of its argument expressions:
            // the callee may also read receiver `self` storage. Admit a footprint
            // only for the exact checked call occurrence at this statement — the
            // same flow/borrow join `structured_call_writes` uses — and only when
            // the typed call carries no machine-valued, requirement, quotient, or
            // private-layout binder that could hand the callee storage the
            // operand scan cannot see. A static type or const application is not
            // such a binder: it substitutes a declaration identity or a
            // compile-time value, so the applied call reads exactly what the same
            // call without the application reads. The checked operand accesses then
            // enumerate every caller place reachable through the arguments,
            // while a `self` target adds the canonical receiver place so an
            // implicit-self callee still reads the caller's machine storage.
            // Targets that do not resolve to a state (requirement symbols,
            // boundary signatures, intrinsics) keep the footprint incomplete.
            ExpressionNode::Call(call) => {
                let Some(calls) = self.calls else {
                    return false;
                };
                let site = CallSite::Expression { expression, call };
                let Some(borrow_call) =
                    calls.find_call(program, machine, state, statement_index, &site)
                else {
                    return false;
                };
                if call.static_machine_parameter.is_valid()
                    || !call.selects_only_nominal_route()
                    || !static_application_carries_no_caller_storage(program, call)
                {
                    return false;
                }
                if call.receiver.is_valid() && !self.collect_reads(call.receiver, depth + 1) {
                    return false;
                }
                for argument in program.expression_table.expression_handles(call.arguments) {
                    if !self.collect_reads(*argument, depth + 1) {
                        return false;
                    }
                }
                for mut place in calls.operand_access_places(borrow_call) {
                    if self.validate_place_read(&mut place).is_none() {
                        return false;
                    }
                    if !self.reads.contains(&place) {
                        self.reads.push(place);
                    }
                }
                // Named calls may target the machine symbol rather than a state
                // symbol; the contract lookup resolves both forms to the exact
                // invocation while requirement, trait-signature, and conformance
                // targets resolve to symbols that own no state and stay opaque.
                let Some((target_machine_symbol, target_state_symbol)) =
                    crate::proof::contract_target_from_state_symbol(
                        program,
                        borrow_call.target_symbol,
                    )
                else {
                    return false;
                };
                let Some(_target_state) = validation::find_state_in_machine(
                    program,
                    target_machine_symbol,
                    target_state_symbol,
                ) else {
                    return false;
                };
                true
            }
            ExpressionNode::Name(_) => self.collect_place_read(expression, depth),
            ExpressionNode::Member(member) => self.collect_member_reads(expression, member, depth),
            // Builtin `items[i]`/`items[a..b]` syntax projects element or window
            // storage and keeps the ordinary place footprint while the chain
            // bottoms out at caller storage. A collection that produces a
            // temporary instead — `compute(seed).a[i]`, `Pair { .. }.a[i]`,
            // `compute()[i]` — owns no element place at all, so its complete
            // footprint is whatever producing that collection read plus the
            // selector operand reads under the same builtin bound floor the
            // place scan applies. Once an authored `[]`/`[..]` declaration
            // governs the occurrence the application is call-shaped instead:
            // its complete footprint is the operand reads authenticated by the
            // exact checked use row, so a missing or unstable selection stays
            // incomplete rather than pretending to be element storage.
            ExpressionNode::Indexed(indexed) => {
                if has_builtin_index_meaning(
                    program,
                    machine,
                    state,
                    statement_index,
                    expression,
                    indexed,
                ) {
                    // Place-rootedness is hereditary: the canonical root comes
                    // from the leftmost leaf of the selector chain, so the whole
                    // expression's root decides which footprint applies — the
                    // same split `collect_member_reads` makes for temporary
                    // receivers.
                    if let Some(place) = canonical_place_from_expression_in_state(
                        program,
                        state.symbol,
                        statement_index,
                        expression,
                    ) && matches!(place.root, facts::PlaceRoot::Symbol(_))
                    {
                        self.descend_and_capture_symbol_place(expression, place, depth)
                    } else {
                        self.collect_reads(indexed.collection, depth + 1)
                            && self.collect_operand_reads(indexed.index, depth)
                    }
                } else {
                    self.collect_selected_index_reads(expression, indexed, depth)
                }
            }
            // `a..b`, `a..`, `..b`, and `..` evaluate only their present bounds.
            // An omitted open bound reads nothing; a present bound is an operand
            // position that must keep builtin bound meaning, exactly like the
            // selector scan requires of a point index.
            ExpressionNode::Range(range) => {
                (!range.start.is_valid() || self.collect_operand_reads(range.start, depth))
                    && (!range.end.is_valid() || self.collect_operand_reads(range.end, depth))
            }
            // Every array literal element is evaluated at the literal site.
            ExpressionNode::ArrayLiteral(elements) => program
                .expression_table
                .expression_handles(*elements)
                .iter()
                .all(|element| self.collect_operand_reads(*element, depth)),
            // Authored and synthesized erased-field initializers alike are
            // evaluated where the literal appears.
            ExpressionNode::StructLiteral(literal) => program
                .expression_table
                .struct_fields(literal.fields)
                .iter()
                .all(|field| self.collect_operand_reads(field.value, depth)),
            // A match reads its subject, every pattern compared against it, and
            // the selected arm's value. Unioning all arms is the conservative
            // footprint: an unselected arm can only shrink the storage the
            // result depends on, never grow it.
            ExpressionNode::Match(dispatch) => {
                self.collect_operand_reads(dispatch.subject, depth)
                    && program.expression_table.match_arms_all(
                        dispatch.arms,
                        |arm_pattern, arm_value| {
                            (match arm_pattern {
                                typed_trees::expression::MatchPattern::Destructure(_) => program
                                    .expression_table
                                    .match_pattern_operands(arm_pattern)
                                    .all(|operand| self.collect_operand_reads(operand, depth)),
                                typed_trees::expression::MatchPattern::Value(pattern) => {
                                    self.collect_operand_reads(pattern, depth)
                                }
                                typed_trees::expression::MatchPattern::Wildcard => true,
                            }) && self.collect_operand_reads(arm_value, depth)
                        },
                    )
            }
            // `place.load(ordering)` is the one atomic observation whose complete
            // footprint is exactly its resident place: the desugar keeps that
            // place in `value` and leaves `result` empty. Every writing axis is
            // instead an assignment carrier (`target = Atomic { .. }`) whose
            // `value` wraps a stored operand or an instruction-shaped update, so
            // the resident place is the carrier statement's `target`, not a child
            // of `value`. A writing axis is admitted only with its complete
            // footprint: the carrier at this statement supplying the resident
            // place, every stored-operand read, and — for the axes that observe a
            // prior — a `result` destination naming current local storage. The
            // parser validates orderings and builds the desugar shape, and typing
            // rejects custody that disagrees with the ordering axis; this walk
            // reads them as given.
            ExpressionNode::Atomic(atomic) => {
                let footprint_start = self.reads.len();
                match atomic.ordering {
                    language_core::atomic::AtomicOrderingPlan::Load(_) => {
                        self.collect_reads(atomic.value, depth + 1)
                            && self.reads.len() > footprint_start
                    }
                    _ => self.collect_atomic_write_reads(expression, atomic, depth),
                }
            }
        }
    }
}

//! The storage a call or local state jump may read through its data binding.
//!
//! A whole `&mut` receiver is judged by its footprint, not as one exclusive
//! operand, so arguments lending disjoint fields stay usable beside it. The
//! mutation summaries supply what the callee writes through the binding; this
//! module supplies what it reads. An exclusive argument loan suspends the
//! receiver branch it lends for the whole invocation, so a callee read of
//! that branch through `self` observes storage another live path may rewrite.
//!
//! A state's footprint is a may-access set in its own coordinates: every
//! place reached through the exact data parameter or attached storage that the body names as a
//! read operand, borrow target, call receiver, call or transition operand,
//! or index selector. A plain assignment's written place is not a read; the
//! mutation summary owns it. Naming a place covers every later access
//! through a reference derived from it, because such a reference cannot
//! reach storage outside the named place. Proof facts are not runtime
//! accesses.
//!
//! A call whose target takes a receiver reached through `self` contributes
//! that target's footprint instantiated at the receiver place, so
//! `self.helper()` reads only what `helper` reads and a jump to a sibling
//! state reads only what that state reads. A target without a summarized
//! body, or a read that cannot be placed at the call, contributes the whole
//! receiver place instead, which covers everything the target can reach
//! through it. A state whose own body or receiver call cannot be placed has
//! no footprint, and its callers treat their whole receiver as read.
//!
//! Local jumps obtain a named data binding from its actual argument at the
//! declared ordinal. Transporting that reference delegates observation to the
//! successor footprint; its selectors still execute in the source state.
//! Ordinary explicit machine-call arguments keep their whole-operand reads.
//! Named parameter roots never collapse to the machine symbol: two instances
//! of the same record must keep distinct storage identities.
//!
//! Each state keeps whole-run and per-parameter footprints (`jump_loans`).
//! The whole run includes every state it jumps to, because a call holds its
//! argument loans until it returns. Each held footprint serves a jump into
//! the state and ends when that parameter's loans stop being forwarded. It
//! includes the whole run of each returning call and the relevant parameter
//! footprints of each successor, not those of independently forwarded loans.
//!
//! Recursive states settle by a monotone fixed point: footprints only grow.
//! An instantiated place deeper than `MAXIMUM_PLACE_DEPTH` widens to its
//! call's receiver place, which bounds the place set so the fixed point
//! terminates even through self-referential projections.

use validation::CanonicalPlace;
use validation::canonical_place_from_expression_in_state;
use validation::{CallSite, find_call_site_in_state};

use crate::flow::jump_loans::{HeldAccesses, TargetRun, held_accesses, target_run};
use crate::flow::mutation::{
    WritePlaceNamespace, call_receiver_mutated_place, canonical_receiver_place_for_call_site,
    instantiate_call_relative_places, local_jump_data_argument,
};

use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::{BorrowCallFact, BorrowFacts};
use symbols::SymbolHandle;
use typed_trees::statement::StatementRootRole;

/// Deeper instantiated reads widen to their call's receiver place.
const MAXIMUM_PLACE_DEPTH: usize = 32;

#[derive(Debug, Default)]
pub(crate) struct StateReceiverReadCache {
    // Footprints depend only on the immutable program and borrow facts, and
    // most programs never ask: build on first use, after the fixed point.
    states: std::sync::OnceLock<Vec<StateReceiverReads>>,
}

#[derive(Debug)]
struct StateReceiverReads {
    state_symbol: SymbolHandle,
    /// False when the body or one of its receiver calls cannot be placed.
    complete: bool,
    /// The whole run's reads.
    reads: Vec<CanonicalPlace>,
    /// Reads while each incoming parameter's loans remain held.
    held_reads: Vec<HeldAccesses<CanonicalPlace>>,
}

/// The places `borrow_call`'s target may read through its receiver while
/// `borrow_call`'s own argument loans are live, in the caller's coordinates:
/// the target's whole-run footprint for a call or its held-run footprint for
/// a jump, instantiated at the receiver, or the whole receiver place when no
/// footprint is known or a read cannot be placed. Empty when the receiver
/// itself has no place.
pub(crate) fn call_receiver_read_accesses(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    cache: &StateReceiverReadCache,
    loan: Option<&CanonicalPlace>,
) -> Vec<CanonicalPlace> {
    let whole_receiver = || {
        call_receiver_mutated_place(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call,
        )
        .map(|place| vec![place])
        .unwrap_or_default()
    };
    let Some(target) = program.state_by_symbol(borrow_call.target_symbol) else {
        return whole_receiver();
    };
    let Some(footprint) = cache
        .states
        .get_or_init(|| build_state_receiver_reads(program, borrow))
        .iter()
        .find(|entry| entry.state_symbol == target.symbol)
        .filter(|entry| entry.complete)
    else {
        return whole_receiver();
    };
    let run = target_run(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
    );
    let reads = run.accesses(program, &footprint.reads, &footprint.held_reads, loan);
    let mut instantiated = Vec::new();
    for read in reads {
        let Some(places) = instantiate_call_relative_places(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow,
            borrow_call,
            read,
            WritePlaceNamespace::AccessRoute,
            None,
        ) else {
            return whole_receiver();
        };
        for place in places {
            if !instantiated.contains(&place) {
                instantiated.push(place);
            }
        }
    }
    instantiated
}

/// One call in a state whose target takes a receiver reached through that
/// state's own receiver.
struct ReceiverCall<'facts> {
    call: &'facts BorrowCallFact,
    receiver: CanonicalPlace,
    /// The target's footprint position; `None` for a target without one.
    target: Option<usize>,
    /// The part of the target's run the owner's held run includes.
    run: TargetRun,
}

/// The state a footprint belongs to, resolved once for the fixed point.
struct FootprintOwner<'program> {
    machine_symbol: SymbolHandle,
    state: &'program typed_trees::state::State,
}

fn build_state_receiver_reads(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
) -> Vec<StateReceiverReads> {
    let mut footprints = Vec::new();
    let mut owners = Vec::new();
    let mut receiver_calls = Vec::new();
    for borrow_state in borrow.states.iter().map(|(_, state)| state) {
        let Some(machine) = program.machine_by_symbol(borrow_state.machine_symbol) else {
            continue;
        };
        let Some(state) = program.state_in_machine(machine, borrow_state.state_symbol) else {
            continue;
        };
        let owner = FootprintOwner {
            machine_symbol: machine.symbol,
            state,
        };
        let mut complete = machine.body_is_present
            && program
                .state_parameters(state)
                .iter()
                .all(|parameter| parameter.type_reference.is_valid());
        let mut delegated = DelegatedReceivers::default();
        let mut calls = Vec::new();
        for call in borrow.calls.span_or_empty(borrow_state.calls) {
            let Some(site) = find_call_site_in_state(
                program,
                machine,
                state,
                call.statement_index,
                call.call_ordinal,
            ) else {
                complete = false;
                continue;
            };
            let data_argument = local_jump_data_argument(program, machine.symbol, &site);
            if data_argument.is_none() {
                continue;
            }
            match site {
                CallSite::Statement(_) => delegated.statements.push(call.statement_index),
                CallSite::Expression { expression, .. } => {
                    delegated.expressions.push(expression);
                }
                CallSite::TransitionNamed { .. } => {
                    if let Some(argument) = data_argument {
                        delegated
                            .transition_arguments
                            .push((call.statement_index, argument));
                    }
                }
            }
            let Some(receiver) = canonical_receiver_place_for_call_site(
                program,
                machine.symbol,
                state.symbol,
                &site,
                call.statement_index,
            ) else {
                complete = false;
                continue;
            };
            if owner.reaches(program, &receiver) {
                calls.push(ReceiverCall {
                    call,
                    receiver,
                    target: None,
                    run: target_run(program, machine.symbol, state.symbol, borrow, call),
                });
            }
        }
        let mut reads = Vec::new();
        if complete {
            DirectReads {
                program,
                owner: &owner,
                statement_index: 0,
                delegated: &delegated,
                reads: &mut reads,
            }
            .collect();
        }
        footprints.push(StateReceiverReads {
            state_symbol: state.symbol,
            complete,
            held_reads: held_accesses(program, state, &reads),
            reads,
        });
        owners.push(owner);
        receiver_calls.push(calls);
    }
    for calls in &mut receiver_calls {
        for receiver_call in calls {
            receiver_call.target = program
                .state_by_symbol(receiver_call.call.target_symbol)
                .and_then(|target| {
                    footprints
                        .iter()
                        .position(|footprint| footprint.state_symbol == target.symbol)
                });
        }
    }
    settle_receiver_call_reads(program, borrow, &owners, &receiver_calls, &mut footprints);
    footprints
}

/// Adds each receiver call's instantiated target footprints until no
/// footprint grows. Unions are monotone, so the visiting order does not
/// change the settled sets.
fn settle_receiver_call_reads(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    owners: &[FootprintOwner<'_>],
    receiver_calls: &[Vec<ReceiverCall<'_>>],
    footprints: &mut [StateReceiverReads],
) {
    loop {
        let mut grew = false;
        for (index, owner) in owners.iter().enumerate() {
            if !footprints[index].complete {
                continue;
            }
            let mut additions = Vec::new();
            let mut held_additions = vec![Vec::new(); footprints[index].held_reads.len()];
            for receiver_call in &receiver_calls[index] {
                let target = receiver_call
                    .target
                    .map(|target| &footprints[target])
                    .filter(|target| target.complete);
                let whole = owner.instantiate(
                    program,
                    borrow,
                    receiver_call,
                    target.map(|target| target.reads.as_slice()),
                );
                for (incoming, additions) in
                    footprints[index].held_reads.iter().zip(&mut held_additions)
                {
                    let loan = CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(incoming.parameter),
                        segments: Vec::new(),
                    };
                    let selected = target.map(|target| {
                        receiver_call
                            .run
                            .accesses(program, &target.reads, &target.held_reads, Some(&loan))
                            .cloned()
                            .collect::<Vec<_>>()
                    });
                    additions.extend(owner.instantiate(
                        program,
                        borrow,
                        receiver_call,
                        selected.as_deref(),
                    ));
                }
                additions.extend(whole);
            }
            let footprint = &mut footprints[index];
            grew |= extend_new(&mut footprint.reads, additions);
            for (held, additions) in footprint.held_reads.iter_mut().zip(held_additions) {
                grew |= extend_new(&mut held.accesses, additions);
            }
        }
        if !grew {
            return;
        }
    }
}

/// Appends the places `reads` lacks; whether any was new.
fn extend_new(reads: &mut Vec<CanonicalPlace>, additions: Vec<CanonicalPlace>) -> bool {
    let mut grew = false;
    for place in additions {
        if !reads.contains(&place) {
            reads.push(place);
            grew = true;
        }
    }
    grew
}

impl FootprintOwner<'_> {
    /// One target footprint at `receiver_call`, in this owner's coordinates.
    /// An unknown footprint or an unplaceable read widens to the call's
    /// receiver place; so does an instantiated place deeper than
    /// `MAXIMUM_PLACE_DEPTH`.
    fn instantiate(
        &self,
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        receiver_call: &ReceiverCall<'_>,
        target_reads: Option<&[CanonicalPlace]>,
    ) -> Vec<CanonicalPlace> {
        let instantiated = target_reads.and_then(|reads| {
            let mut places = Vec::new();
            for read in reads {
                places.extend(instantiate_call_relative_places(
                    program,
                    self.machine_symbol,
                    self.state.symbol,
                    borrow,
                    receiver_call.call,
                    read,
                    WritePlaceNamespace::AccessRoute,
                    None,
                )?);
            }
            Some(places)
        });
        let Some(places) = instantiated else {
            return vec![receiver_call.receiver.clone()];
        };
        let mut owned = Vec::new();
        for place in places {
            if !self.reaches(program, &place) {
                continue;
            }
            owned.push(if place.segments.len() > MAXIMUM_PLACE_DEPTH {
                receiver_call.receiver.clone()
            } else {
                place
            });
        }
        owned
    }

    /// Whether `place` is storage this state reaches through its data binding.
    /// Named roots stay distinct from the machine and from other records of
    /// the same type; instantiation supplies their exact caller-side identity.
    fn reaches(&self, program: &dyn ::validation::CheckingInput, place: &CanonicalPlace) -> bool {
        let Some(root) = place.root.as_symbol() else {
            return false;
        };
        root == self.machine_symbol
            || program
                .machine_by_symbol(self.machine_symbol)
                .and_then(|machine| {
                    program.data_binding_parameter(machine, program.state_parameters(self.state))
                })
                .is_some_and(|parameter| parameter.symbol == root)
    }
}

/// Calls whose receiver footprint comes from the fixed point instead of the
/// receiver operand itself.
#[derive(Default)]
struct DelegatedReceivers {
    expressions: Vec<ExpressionHandle>,
    statements: Vec<usize>,
    transition_arguments: Vec<(usize, ExpressionHandle)>,
}

/// One state's directly named receiver places.
struct DirectReads<'walk, 'program> {
    program: &'program dyn ::validation::CheckingInput,
    owner: &'walk FootprintOwner<'program>,
    statement_index: usize,
    delegated: &'walk DelegatedReceivers,
    reads: &'walk mut Vec<CanonicalPlace>,
}

impl DirectReads<'_, '_> {
    fn collect(&mut self) {
        let program = self.program;
        for (statement_index, statement) in program
            .statement_table
            .statements(self.owner.state.statement_nodes)
            .iter()
            .enumerate()
        {
            self.statement_index = statement_index;
            if let Some(call) = statement.as_call()
                && !self.delegated.statements.contains(&statement_index)
                && let Some(receiver) = canonical_receiver_place_for_call_site(
                    program,
                    self.owner.machine_symbol,
                    self.owner.state.symbol,
                    &CallSite::Statement(call),
                    statement_index,
                )
            {
                self.record(receiver);
            }
            for (role, root) in program.tables.expression_roots(statement) {
                if !root.is_valid() {
                    continue;
                }
                match role {
                    StatementRootRole::AssemblyFact => {}
                    StatementRootRole::AssignmentTarget => self.place(root, false),
                    StatementRootRole::TransitionArgument
                        if self
                            .delegated
                            .transition_arguments
                            .contains(&(statement_index, root)) =>
                    {
                        // Forwarding the parent reference does not observe its
                        // entire referent. The successor's held footprint owns
                        // those reads; evaluating projection selectors still
                        // observes their operands here.
                        self.place(root, false);
                    }
                    _ => self.value(root),
                }
            }
        }
    }

    fn record(&mut self, place: CanonicalPlace) {
        if self.owner.reaches(self.program, &place) && !self.reads.contains(&place) {
            self.reads.push(place);
        }
    }

    /// An operand evaluated for its value.
    fn value(&mut self, expression: ExpressionHandle) {
        if !expression.is_valid() {
            return;
        }
        let program = self.program;
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
                self.place(expression, true);
            }
            ExpressionNode::Borrow(inner) => self.place(inner.target, true),
            ExpressionNode::Cast(cast) => self.value(cast.value),
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    let delegated = self.delegated.expressions.contains(&expression);
                    self.place(call.receiver, !delegated);
                }
                for argument in program.expression_table.expression_handles(call.arguments) {
                    self.value(*argument);
                }
            }
            ExpressionNode::Match(dispatch) => {
                self.value(dispatch.subject);
                for arm in program.expression_table.match_arms(dispatch.arms) {
                    if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                        self.value(pattern);
                    }
                    self.value(arm.value);
                }
            }
            ExpressionNode::Atomic(atomic) => self.value(atomic.value),
            ExpressionNode::ArrayLiteral(values) => {
                for value in program.expression_table.expression_handles(*values) {
                    self.value(*value);
                }
            }
            ExpressionNode::Range(range) => {
                self.value(range.start);
                self.value(range.end);
            }
            ExpressionNode::Binary(binary) => {
                self.value(binary.left);
                self.value(binary.right);
            }
            ExpressionNode::Unary(unary) => self.value(unary.operand),
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    self.value(field.value);
                }
            }
            typed_trees::spelled_literal_expression_pattern!() => {}
        }
    }

    /// An operand evaluated for its storage. Its selectors are always read;
    /// `named` records the place itself.
    fn place(&mut self, expression: ExpressionHandle, named: bool) {
        if !expression.is_valid() {
            return;
        }
        if let Some(place) = canonical_place_from_expression_in_state(
            self.program,
            self.owner.state.symbol,
            self.statement_index,
            expression,
        )
        .filter(|place| matches!(place.root, facts::PlaceRoot::Symbol(_)))
        {
            if named {
                self.record(place);
            }
            self.selectors(expression);
            return;
        }
        // A computed base: its own operands are ordinary reads.
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Member(member) => self.place(member.receiver, named),
            ExpressionNode::Indexed(indexed) => {
                self.place(indexed.collection, named);
                self.value(indexed.index);
            }
            ExpressionNode::Borrow(inner) => self.place(inner.target, named),
            ExpressionNode::Cast(cast) => self.place(cast.value, named),
            ExpressionNode::Name(_) => {}
            _ => self.value(expression),
        }
    }

    fn selectors(&mut self, expression: ExpressionHandle) {
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Member(member) => self.selectors(member.receiver),
            ExpressionNode::Indexed(indexed) => {
                self.selectors(indexed.collection);
                self.value(indexed.index);
            }
            ExpressionNode::Borrow(inner) => self.selectors(inner.target),
            ExpressionNode::Cast(cast) => self.selectors(cast.value),
            _ => {}
        }
    }
}

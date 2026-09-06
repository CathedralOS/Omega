//! State arrivals use the source evaluation environment and the exact jump
//! argument telescope. Each round starts from an overapproximation, including
//! every backedge; stopping before convergence only loses precision.

use super::*;
use crate::CallFrameResolver;
use symbols::SymbolHandle;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

/// Bound one exact terminal expression using every arrival and the effects of
/// its state prefix. Authored requirements are assumptions of body checking;
/// call/transition validation must independently discharge those requirements.
pub fn arrival_integer_expression_bounds(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<(i64, i64)> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    if !matches!(statements.get(statement_index), Some(StatementNode::Expression(value)) if *value == expression)
    {
        return None;
    }
    let mut environment = incoming_guard_env(program, machine, state);
    seed_state_requirements(program, machine, state, &mut environment);
    let frames = CallFrameResolver::new(program);
    let mut walk = ArrivalWalk {
        program,
        machine,
        frames: frames.as_ref(),
        joined: vec![None; program.machine_states(machine).len()],
    };
    if !walk.statements(state, &statements[..statement_index], &mut environment) {
        return None;
    }
    walk.expression(state, expression, &mut environment);
    // Within the consuming expression, an aggregate effect frame also covers
    // reads evaluated before a later mutating child.
    walk.expression_effects(expression, &mut environment);
    let interval = walk.interval(state, expression, &environment);
    let (low, high) = (interval.low?, interval.high?);
    (low <= high).then_some((low, high))
}

pub(super) fn seed_state_requirements(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    environment: &mut ValueEnv,
) {
    let is_entry = program
        .machine_states(machine)
        .first()
        .is_some_and(|entry| entry.symbol == state.symbol);
    let mut required = ValueEnv::new();
    for contract in program
        .machine_contracts(machine)
        .iter()
        .filter(|_| is_entry)
        .chain(program.state_contracts(state))
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
    {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let ProofFact::Expression(condition) = fact
                && condition_belongs_to_state(program, state, *condition)
            {
                narrow_env_by_condition(
                    program,
                    machine,
                    Some(state),
                    environment,
                    *condition,
                    true,
                );
                narrow_env_by_condition(
                    program,
                    machine,
                    Some(state),
                    &mut required,
                    *condition,
                    true,
                );
            }
        }
    }
    if environment.intervals.values().any(|interval| {
        interval
            .low
            .zip(interval.high)
            .is_some_and(|(low, high)| low > high)
    }) {
        // An arrival violating requires is rejected by the call-contract
        // checker. Check this body's return under its authored assumptions,
        // without using an empty interval as evidence for a produced value.
        *environment = required;
    }
}

fn condition_belongs_to_state(
    program: &TypedTrees,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    if literal_i64(program, expression).is_some() {
        return true;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            path.symbol.is_valid()
                && path.head_symbol.is_valid()
                && program
                    .state_parameters(state)
                    .iter()
                    .any(|parameter| parameter.symbol == path.head_symbol)
        }
        ExpressionNode::Member(member) => {
            member.member_symbol.is_valid()
                && condition_belongs_to_state(program, state, member.receiver)
        }
        ExpressionNode::Binary(binary) => {
            condition_belongs_to_state(program, state, binary.left)
                && condition_belongs_to_state(program, state, binary.right)
        }
        ExpressionNode::Unary(unary) => condition_belongs_to_state(program, state, unary.operand),
        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) | ExpressionNode::Float(_) => true,
        _ => false,
    }
}

pub(super) fn incoming_environments(
    program: &TypedTrees,
    machine: &Machine,
) -> Vec<(SymbolHandle, ValueEnv)> {
    let states = program.machine_states(machine);
    let mut current = states
        .iter()
        .map(|state| (state.symbol, ValueEnv::new()))
        .collect::<Vec<_>>();
    let frames = CallFrameResolver::new(program);
    // One round per state propagates acyclic chains. Cycles also contribute in
    // every round, starting with their full declared parameter domains. There
    // is no assumption that a seed guard is an inductive loop invariant.
    for _ in 0..=states.len() {
        let mut walk = ArrivalWalk {
            program,
            machine,
            frames: frames.as_ref(),
            joined: vec![None; states.len()],
        };
        if let Some(entry) = states.first() {
            let mut external = ValueEnv::new();
            seed_state_requirements(program, machine, entry, &mut external);
            walk.joined[0] = Some(external);
        }
        for (state, (_, environment)) in states.iter().zip(&current) {
            let mut environment = environment.clone();
            seed_state_requirements(program, machine, state, &mut environment);
            walk.statements(
                state,
                program.statement_table.statements(state.statement_nodes),
                &mut environment,
            );
        }
        let next = states
            .iter()
            .zip(walk.joined)
            .map(|(state, environment)| (state.symbol, environment.unwrap_or_default()))
            .collect::<Vec<_>>();
        if next == current {
            return next;
        }
        current = next;
    }
    current
}

struct ArrivalWalk<'program, 'frames> {
    program: &'program TypedTrees,
    machine: &'program Machine,
    frames: Option<&'frames CallFrameResolver<'program>>,
    joined: Vec<Option<ValueEnv>>,
}

impl ArrivalWalk<'_, '_> {
    fn join(&mut self, symbol: SymbolHandle, environment: ValueEnv) {
        if !symbol.is_valid() {
            return;
        }
        let Some(index) = self
            .program
            .machine_states(self.machine)
            .iter()
            .position(|state| state.symbol == symbol)
        else {
            return;
        };
        self.joined[index] = Some(match &self.joined[index] {
            Some(previous) => previous.join(&environment),
            None => environment,
        });
    }

    fn interval(
        &self,
        source: &State,
        expression: ExpressionHandle,
        environment: &ValueEnv,
    ) -> Interval {
        let mut diagnostics = Vec::new();
        let value = analyze(
            self.program,
            self.machine,
            Some(source),
            expression,
            environment,
            None,
            ArithmeticDomain::Exact,
            "state arrival",
            &mut diagnostics,
        );
        if diagnostics.is_empty() {
            value.interval
        } else {
            Interval::UNBOUNDED
        }
    }

    fn cross_writes(environment: &mut ValueEnv, written: Option<Vec<String>>) {
        if let Some(written) = written {
            environment.invalidate_written_paths(&written);
        } else {
            environment.clear();
        }
    }

    fn expression_effects(&self, expression: ExpressionHandle, environment: &mut ValueEnv) {
        Self::cross_writes(
            environment,
            self.frames
                .and_then(|frames| frames.expression_may_write_paths(self.machine, expression)),
        );
    }

    fn arguments(
        &mut self,
        source: &State,
        arguments: &[ExpressionHandle],
        environment: &mut ValueEnv,
    ) -> Vec<Interval> {
        arguments
            .iter()
            .map(|argument| {
                self.expression(source, *argument, environment);
                self.interval(source, *argument, environment)
            })
            .collect()
    }

    fn arrive(
        &mut self,
        source: &State,
        symbol: SymbolHandle,
        arguments: &[ExpressionHandle],
        intervals: &[Interval],
        environment: &ValueEnv,
    ) {
        let Some(target) = self
            .program
            .machine_states(self.machine)
            .iter()
            .find(|state| state.symbol == symbol && symbol.is_valid())
        else {
            return;
        };
        let parameters = self.program.state_parameters(target);
        if parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count()
            != arguments.len()
        {
            self.join(symbol, ValueEnv::new());
            return;
        }
        let mut bindings = Vec::new();
        if let Some(target_self) = parameters.iter().find(|parameter| parameter.is_self)
            && let Some(source_self) = self
                .program
                .state_parameters(source)
                .iter()
                .find(|parameter| parameter.is_self)
        {
            bindings.push((
                source_self.name.as_str().to_owned(),
                target_self.name.as_str().to_owned(),
            ));
        }
        for (parameter, argument) in parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .zip(arguments)
        {
            if let Some(path) = self.bound_place(source, *argument) {
                bindings.push((path, parameter.name.as_str().to_owned()));
            }
        }
        let mut rebound = environment.rebind(&bindings);
        for (parameter, interval) in parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .zip(intervals)
        {
            // A scalar value is a snapshot taken at its own argument position.
            // Reference parameters retain only facts surviving later arguments.
            if !is_reference(self.program, parameter.type_reference) {
                rebound.set(parameter.name.as_str().to_owned(), *interval);
            }
        }
        self.join(symbol, rebound);
    }

    fn bound_place(&self, source: &State, expression: ExpressionHandle) -> Option<String> {
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Name(path) if path.symbol.is_valid() && path.head_symbol.is_valid() => {
                let root = self
                    .program
                    .state_parameters(source)
                    .iter()
                    .any(|parameter| parameter.symbol == path.head_symbol)
                    || self
                        .program
                        .statement_table
                        .statements(source.statement_nodes)
                        .iter()
                        .any(|statement| {
                            matches!(statement, StatementNode::LocalData(local)
                            if local.symbol == path.head_symbol)
                        });
                root.then(|| place_path(self.program, expression)).flatten()
            }
            ExpressionNode::Member(member) if member.member_symbol.is_valid() => {
                self.bound_place(source, member.receiver)?;
                place_path(self.program, expression)
            }
            ExpressionNode::Borrow(borrow) => self.bound_place(source, borrow.target),
            _ => None,
        }
    }

    fn target(
        &mut self,
        source: &State,
        target: typed_trees::statement::TransitionTargetHandle,
        environment: &mut ValueEnv,
    ) {
        if !target.is_valid() {
            return;
        }
        match self.program.statement_table.transition_target(target) {
            TransitionTargetNode::Named {
                path, arguments, ..
            } => {
                let arguments = self.program.statement_table.expression_handles(*arguments);
                let intervals = self.arguments(source, arguments, environment);
                self.arrive(source, path.symbol, arguments, &intervals, environment);
            }
            TransitionTargetNode::Value(expression) => {
                self.expression(source, *expression, environment)
            }
            TransitionTargetNode::SelfTarget => self.join(source.symbol, environment.clone()),
            TransitionTargetNode::Terminal => {}
        }
    }

    fn statements(
        &mut self,
        source: &State,
        statements: &[StatementNode],
        environment: &mut ValueEnv,
    ) -> bool {
        for statement in statements {
            match statement {
                StatementNode::Transition(transition) => {
                    let mut selected = environment.clone();
                    let mut fallback = environment.clone();
                    if let TransitionGuardNode::When(condition) = transition.guard {
                        // Discover calls at their evaluation positions. Guard
                        // premises are then invalidated by the whole guard's
                        // effects, as in transition argument validation.
                        self.expression(source, condition, environment);
                        narrow_env_by_condition(
                            self.program,
                            self.machine,
                            Some(source),
                            &mut selected,
                            condition,
                            true,
                        );
                        narrow_env_by_condition(
                            self.program,
                            self.machine,
                            Some(source),
                            &mut fallback,
                            condition,
                            false,
                        );
                        self.expression_effects(condition, &mut selected);
                        self.expression_effects(condition, &mut fallback);
                    }
                    self.target(source, transition.target, &mut selected);
                    self.target(source, transition.continuation, &mut fallback);
                    if transition.continuation.is_valid()
                        || transition.guard == TransitionGuardNode::Always
                    {
                        return false;
                    }
                    *environment = fallback;
                }
                StatementNode::LocalData(local) => {
                    self.expression(source, local.initial_value, environment);
                    let interval = self.interval(source, local.initial_value, environment);
                    let path = local.name.as_str().to_owned();
                    environment.invalidate_written_paths(std::slice::from_ref(&path));
                    environment.set(path, interval);
                }
                StatementNode::Assignment(assignment) => {
                    self.expression(source, assignment.target, environment);
                    self.expression(source, assignment.value, environment);
                    // Invalidation is sufficient here. Normal body validation
                    // establishes the new value and checks its store domain.
                    Self::cross_writes(
                        environment,
                        self.frames.and_then(|frames| {
                            frames
                                .assignment_write_frame(self.machine, statement)
                                .into_complete_paths()
                        }),
                    );
                }
                StatementNode::Call(call) => {
                    let arguments = self
                        .program
                        .statement_table
                        .expression_handles(call.arguments);
                    let intervals = self.arguments(source, arguments, environment);
                    self.arrive(
                        source,
                        call.target_symbol,
                        arguments,
                        &intervals,
                        environment,
                    );
                    Self::cross_writes(
                        environment,
                        self.frames
                            .and_then(|frames| frames.may_write_paths(self.machine, call)),
                    );
                }
                StatementNode::Expression(expression) => {
                    self.expression(source, *expression, environment)
                }
                StatementNode::AssemblyFact(_) => {}
            }
        }
        true
    }

    fn expression(
        &mut self,
        source: &State,
        expression: ExpressionHandle,
        environment: &mut ValueEnv,
    ) {
        if !expression.is_valid() {
            return;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Call(call) => {
                self.expression(source, call.receiver, environment);
                let arguments = self
                    .program
                    .expression_table
                    .expression_handles(call.arguments);
                let intervals = self.arguments(source, arguments, environment);
                self.arrive(
                    source,
                    call.target_symbol,
                    arguments,
                    &intervals,
                    environment,
                );
                self.expression_effects(expression, environment);
            }
            ExpressionNode::Binary(binary) => {
                self.expression(source, binary.left, environment);
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                    let skipped = environment.clone();
                    self.expression(source, binary.right, environment);
                    *environment = skipped.join(environment);
                } else {
                    self.expression(source, binary.right, environment);
                }
            }
            ExpressionNode::Unary(unary) => self.expression(source, unary.operand, environment),
            ExpressionNode::Member(member) => self.expression(source, member.receiver, environment),
            ExpressionNode::Indexed(indexed) => {
                self.expression(source, indexed.collection, environment);
                self.expression(source, indexed.index, environment);
            }
            ExpressionNode::Atomic(atomic) => self.expression(source, atomic.value, environment),
            ExpressionNode::Cast(cast) => self.expression(source, cast.value, environment),
            ExpressionNode::Borrow(borrow) => self.expression(source, borrow.target, environment),
            ExpressionNode::Range(range) => {
                self.expression(source, range.start, environment);
                self.expression(source, range.end, environment);
            }
            ExpressionNode::ArrayLiteral(values) => {
                for value in self.program.expression_table.expression_handles(*values) {
                    self.expression(source, *value, environment);
                }
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in self.program.expression_table.struct_fields(literal.fields) {
                    self.expression(source, field.value, environment);
                }
            }
            ExpressionNode::Name(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }
}

fn is_reference(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference { .. } => true,
        TypeReferenceNode::Constrained { base_type, .. } => is_reference(program, *base_type),
        _ => false,
    }
}

//! Current scalar values across ordinary statement transfers.
//!
//! A hypothesis captures its input roster when it is established. Assignment
//! captures the RHS under that roster before replacing its destination; later
//! reads never reinterpret an earlier name as the replacement. This is private
//! proof construction scratch, not an invocation-memory snapshot or a retained
//! certificate. Arrivals, exits and ranks share this transfer. Unknown effects
//! retire current bindings; they cannot revive an entry fact about new storage.
use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetHandle};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};
use typed_trees::{TypedTrees, machine::Machine, state::State};
use validation::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticHypothesis, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
};

#[cfg(test)]
mod tests;

#[derive(Clone)]
struct Binding {
    symbol: SymbolHandle,
    name: String,
    value: Option<ScopedArithmeticValue>,
}

pub(crate) struct CurrentScalarValues<'program> {
    program: &'program TypedTrees,
    machine: &'program Machine,
    state: &'program State,
    initial: Vec<Binding>,
    current: Vec<Binding>,
    hypotheses: Vec<ScopedArithmeticHypothesis>,
}

impl<'program> CurrentScalarValues<'program> {
    pub(crate) fn at(
        program: &'program TypedTrees,
        machine: &'program Machine,
        state: &'program State,
        statement_index: usize,
        target: TransitionTargetHandle,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Option<Self> {
        let initial = program
            .state_parameters(state)
            .iter()
            .filter_map(|parameter| {
                let primitive = exact_integer(program, parameter.type_reference)?;
                Some(Binding {
                    symbol: parameter.symbol,
                    name: parameter.name.to_string(),
                    value: Some(ScopedArithmeticValue::Atom {
                        identity: format!(
                            "state:{:?}:parameter:{:?}",
                            state.symbol, parameter.symbol
                        ),
                        unsigned: unsigned(primitive),
                    }),
                })
            })
            .collect::<Vec<_>>();
        let mut values = Self {
            program,
            machine,
            state,
            current: initial.clone(),
            initial,
            hypotheses: Vec::new(),
        };
        let owned_frames = frames
            .is_none()
            .then(|| validation::CallFrameResolver::new(program))
            .flatten();
        let frames = frames.or(owned_frames.as_ref());
        let entry = program.machine_states(machine).first()?;
        let contracts = program.state_contracts(state).iter().chain(
            program
                .machine_contracts(machine)
                .iter()
                .filter(|_| entry.symbol == state.symbol),
        );
        for contract in contracts {
            if contract.kind != typed_trees::signature::SignatureContractKind::Requires {
                continue;
            }
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                if let typed_trees::domain::ProofFact::Expression(expression) = fact {
                    values.assume(program, machine, state, *expression, true);
                }
            }
        }
        let statements = program.statement_table.statements(state.statement_nodes);
        if statement_index > statements.len() {
            return None;
        }
        for statement in &statements[..statement_index] {
            if validation::is_arm_pattern_marker(statement) {
                continue;
            }
            match statement {
                StatementNode::Transition(transition) => {
                    let TransitionGuardNode::When(guard) = transition.guard else {
                        return None;
                    };
                    values.invalidate_frame(
                        frames.map(|frames| frames.expression_write_frame(machine, guard)),
                    );
                    // A missing primary target preserves its true arm; a
                    // missing continuation preserves its false arm.
                    let holds = match (
                        transition.target.is_valid(),
                        transition.continuation.is_valid(),
                    ) {
                        (true, false) => false,
                        (false, true) => true,
                        _ => return None,
                    };
                    values.assume(program, machine, state, guard, holds);
                }
                StatementNode::LocalData(local) => {
                    values.invalidate_operand_effects(program, machine, statement, frames);
                    let value = values.capture(program, machine, state, local.initial_value);
                    if exact_integer(program, local.type_reference).is_some() {
                        values.current.push(Binding {
                            symbol: local.symbol,
                            name: local.name.to_string(),
                            value,
                        });
                    }
                }
                StatementNode::Assignment(assignment) => {
                    values.invalidate_operand_effects(program, machine, statement, frames);
                    let value = values.capture(program, machine, state, assignment.value);
                    if let ExpressionNode::Name(name) =
                        program.expression_table.expression(assignment.target)
                        && name.symbol.is_valid()
                        && name.symbol == name.head_symbol
                        && let Some(binding) = values
                            .current
                            .iter_mut()
                            .find(|binding| binding.symbol == name.symbol)
                    {
                        binding.value = value;
                    } else {
                        values.invalidate_frame(
                            frames.map(|frames| frames.assignment_write_frame(machine, statement)),
                        );
                    }
                }
                StatementNode::Call(call) => {
                    values.invalidate_operand_effects(program, machine, statement, frames);
                    values.invalidate_frame(
                        frames.map(|frames| frames.may_write_frame(machine, call)),
                    );
                }
                StatementNode::Expression(_) => {
                    values.invalidate_operand_effects(program, machine, statement, frames)
                }
                _ => return None,
            }
        }
        if target.is_valid() {
            let StatementNode::Transition(transition) = statements.get(statement_index)? else {
                return None;
            };
            if transition.target != target && transition.continuation != target {
                return None;
            }
            if let TransitionGuardNode::When(guard) = transition.guard {
                values.invalidate_frame(
                    frames.map(|frames| frames.expression_write_frame(machine, guard)),
                );
                values.assume(program, machine, state, guard, transition.target == target);
            }
        }
        if target.is_valid() {
            if let typed_trees::statement::TransitionTargetNode::Named { arguments, .. } =
                program.statement_table.transition_target(target)
            {
                for argument in program.statement_table.expression_handles(*arguments) {
                    values.invalidate_frame(
                        frames.map(|frames| frames.expression_write_frame(machine, *argument)),
                    );
                }
            }
        }
        Some(values)
    }

    fn invalidate_operand_effects(
        &mut self,
        _program: &TypedTrees,
        machine: &Machine,
        statement: &StatementNode,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) {
        // Complete existing operand frames include nested calls, rather than
        // assuming a syntactic assignment or local declaration is effect-free.
        if let Some(frames) = frames {
            self.invalidate_frame(Some(frames.statement_value_write_frame(machine, statement)));
        } else if statement_has_call(statement) {
            self.invalidate_frame(None);
        }
    }

    fn invalidate_frame(&mut self, frame: Option<facts::NormalizedWriteFrame>) {
        let paths = frame.and_then(|frame| frame.into_complete_paths());
        for binding in &mut self.current {
            if paths.as_ref().is_none_or(|paths| {
                paths
                    .iter()
                    .any(|path| validation::frame_paths_overlap(path, &binding.name))
            }) {
                binding.value = None;
            }
        }
    }

    fn assume(
        &mut self,
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        holds: bool,
    ) {
        self.assume_at_depth(program, machine, state, expression, holds, 0);
    }

    fn assume_at_depth(
        &mut self,
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        holds: bool,
        depth: usize,
    ) {
        if depth >= 64 || !program.expression_table.expression_is_valid(expression) {
            return;
        }
        if let ExpressionNode::Binary(binary) = program.expression_table.expression(expression)
            && matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            )
            && validation::has_builtin_binary_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            )
        {
            let wrapped = match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (_, ExpressionNode::Boolean(value)) => Some((binary.left, *value)),
                (ExpressionNode::Boolean(value), _) => Some((binary.right, *value)),
                _ => None,
            };
            if let Some((operand, value)) = wrapped {
                self.assume_at_depth(
                    program,
                    machine,
                    state,
                    operand,
                    holds == (value == (binary.operator == BinaryOperator::Equal)),
                    depth + 1,
                );
                return;
            }
        }
        if let ExpressionNode::Binary(binary) = program.expression_table.expression(expression)
            && ((holds && binary.operator == BinaryOperator::And)
                || (!holds && binary.operator == BinaryOperator::Or))
            && validation::has_builtin_decomposed_guard_meaning(
                program,
                machine,
                Some(state),
                expression,
            )
        {
            self.assume_at_depth(program, machine, state, binary.left, holds, depth + 1);
            self.assume_at_depth(program, machine, state, binary.right, holds, depth + 1);
            return;
        }
        if let ExpressionNode::Unary(unary) = program.expression_table.expression(expression)
            && unary.operator == typed_trees::expression::UnaryOperator::LogicalNot
            && validation::has_builtin_bound_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            )
        {
            self.assume_at_depth(program, machine, state, unary.operand, !holds, depth + 1);
            return;
        }
        if admitted(program, machine, state, expression, &self.current, 0) {
            if let Some(proposition) = scoped(expression, &self.current, program) {
                self.hypotheses
                    .push(ScopedArithmeticHypothesis { proposition, holds });
            }
        }
    }

    fn capture(
        &self,
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<ScopedArithmeticValue> {
        admitted(program, machine, state, expression, &self.current, 0)
            .then(|| scoped(expression, &self.current, program))
            .flatten()
            .map(ScopedArithmeticValue::Term)
    }

    pub(crate) fn initial_symbol(&self, symbol: SymbolHandle) -> Option<ScopedArithmeticValue> {
        self.initial
            .iter()
            .find(|binding| binding.symbol == symbol)?
            .value
            .clone()
    }
    pub(crate) fn current_symbol(&self, symbol: SymbolHandle) -> Option<ScopedArithmeticValue> {
        self.current
            .iter()
            .find(|binding| binding.symbol == symbol)?
            .value
            .clone()
    }
    pub(crate) fn current(
        &self,
        expression: ExpressionHandle,
    ) -> Option<ScopedArithmeticExpression> {
        admitted(
            self.program,
            self.machine,
            self.state,
            expression,
            &self.current,
            0,
        )
        .then(|| scoped(expression, &self.current, self.program))
        .flatten()
    }
    /// Simultaneous parameter substitution with independently retained actual
    /// snapshots. The caller owns selection and exact formal/actual roster
    /// identity; this owner checks arithmetic meaning before exposing a term.
    pub(crate) fn bind_parameters(
        &self,
        callee: &Machine,
        state: &State,
        expression: ExpressionHandle,
        parameters: &[typed_trees::signature::StateParameter],
        arguments: &[ExpressionHandle],
    ) -> Option<ScopedArithmeticExpression> {
        if parameters.len() != arguments.len() {
            return None;
        }
        let mut bindings = Vec::with_capacity(parameters.len());
        let mut formals = Vec::with_capacity(parameters.len());
        for (parameter, argument) in parameters.iter().zip(arguments) {
            let primitive = exact_integer(self.program, parameter.type_reference)?;
            bindings.push(ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(parameter.symbol),
                value: ScopedArithmeticValue::Term(self.current(*argument)?),
            });
            formals.push(Binding {
                symbol: parameter.symbol,
                name: parameter.name.to_string(),
                value: Some(ScopedArithmeticValue::Atom {
                    identity: format!("callee:{:?}", parameter.symbol),
                    unsigned: unsigned(primitive),
                }),
            });
        }
        admitted(self.program, callee, state, expression, &formals, 0).then_some(
            ScopedArithmeticExpression {
                expression,
                bindings,
            },
        )
    }

    pub(crate) fn proves_values_relation(
        &self,
        program: &TypedTrees,
        machine: &Machine,
        operator: BinaryOperator,
        left: &ScopedArithmeticValue,
        right: &ScopedArithmeticValue,
    ) -> bool {
        validation::scoped_arithmetic_values_relation(
            program,
            machine,
            &self.hypotheses,
            operator,
            left,
            right,
        ) == StrictArithmeticImplicationJudgment::Proven
    }
    pub(crate) fn proves(
        &self,
        program: &TypedTrees,
        machine: &Machine,
        goal: &ScopedArithmeticExpression,
    ) -> bool {
        validation::scoped_arithmetic_implication(program, machine, &self.hypotheses, goal)
            == StrictArithmeticImplicationJudgment::Proven
    }
}

fn exact_integer(program: &TypedTrees, reference: TypeReferenceHandle) -> Option<PrimitiveType> {
    let primitive = program.primitive_type_reference(reference)?;
    if !matches!(
        primitive,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) || program.arithmetic_domain_for_type_reference(reference) != ArithmeticDomain::Exact
    {
        return None;
    }
    Some(primitive)
}
fn unsigned(primitive: PrimitiveType) -> bool {
    matches!(
        primitive,
        PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64
    )
}
fn admitted(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    bindings: &[Binding],
    depth: usize,
) -> bool {
    if depth >= 64 || !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) => true,
        ExpressionNode::Name(name) => {
            (name.symbol == name.head_symbol
                && bindings
                    .iter()
                    .any(|binding| binding.symbol == name.symbol && binding.value.is_some()))
                || validation::reserved_result_owner(program, expression)
                    .is_some_and(|(owner, _)| owner == machine.symbol)
        }
        ExpressionNode::Binary(binary) => {
            matches!(
                binary.operator,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
                    | BinaryOperator::And
                    | BinaryOperator::Or
            ) && validation::has_builtin_binary_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) && admitted(program, machine, state, binary.left, bindings, depth + 1)
                && admitted(program, machine, state, binary.right, bindings, depth + 1)
        }
        _ => false,
    }
}

fn scoped(
    expression: ExpressionHandle,
    bindings: &[Binding],
    program: &TypedTrees,
) -> Option<ScopedArithmeticExpression> {
    let mut symbols = Vec::new();
    let mut pending = vec![(expression, 0)];
    let mut budget = 4096usize;
    while let Some((expression, depth)) = pending.pop() {
        budget = budget.checked_sub(1)?;
        if depth >= 64 || !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(_)
                if program.expression_table.names_reserved_result(expression) => {}
            ExpressionNode::Name(name) if name.symbol == name.head_symbol => {
                if !symbols.contains(&name.symbol) {
                    symbols.push(name.symbol);
                }
            }
            ExpressionNode::Integer(_) => {}
            ExpressionNode::Binary(binary) => {
                pending.push((binary.left, depth + 1));
                pending.push((binary.right, depth + 1));
            }
            _ => return None,
        }
    }
    let mut selected = Vec::new();
    for symbol in symbols {
        let value = bindings
            .iter()
            .find(|binding| binding.symbol == symbol)?
            .value
            .as_ref()?;
        charge(value, &mut budget, 0)?;
        selected.push(ScopedArithmeticBinding {
            binder: ScopedArithmeticBinder::Symbol(symbol),
            value: value.clone(),
        });
    }
    Some(ScopedArithmeticExpression {
        expression,
        bindings: selected,
    })
}
fn charge(value: &ScopedArithmeticValue, budget: &mut usize, depth: usize) -> Option<()> {
    *budget = budget.checked_sub(1)?;
    if depth >= 64 {
        return None;
    }
    if let ScopedArithmeticValue::Term(term)
    | ScopedArithmeticValue::AdmittedIntegerRemainder(term) = value
    {
        for binding in &term.bindings {
            charge(&binding.value, budget, depth + 1)?;
        }
    }
    Some(())
}

fn statement_has_call(statement: &StatementNode) -> bool {
    // With no complete operand frame, no expression-bearing statement grants
    // preservation. Pure assignments are handled only with an actual frame.
    matches!(
        statement,
        StatementNode::Call(_)
            | StatementNode::Assignment(_)
            | StatementNode::LocalData(_)
            | StatementNode::Expression(_)
    )
}

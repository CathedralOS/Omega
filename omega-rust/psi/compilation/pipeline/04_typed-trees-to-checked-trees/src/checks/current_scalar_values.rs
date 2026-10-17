//! Current scalar values and descriptor measures across statement transfers.
//!
//! A hypothesis captures its input roster when it is established. Assignment
//! captures the RHS under that roster before replacing its destination; later
//! reads never reinterpret an earlier name as the replacement. This is private
//! proof construction scratch, not an invocation-memory snapshot or a retained
//! certificate. Arrivals, exits and ranks share this transfer. Unknown effects
//! retire current bindings; they cannot revive an entry fact about new storage.
//!
//! A fixed array's extent is metadata for every reference access and every
//! qualification. A stable slice descriptor retains its observed length under
//! the existing access and effect checks. Local descriptors use the same
//! replacement and effect retirement as scalar bindings; their captured
//! measures survive endpoint stores, never descriptor replacement.
//!
//! A match result has one length only when every arm's length is the same
//! observation, or each arm folds to the same exact integer. Differing arms
//! stay unknown rather than publishing one arm's extent. A plain record
//! value captures each slice field's length when that value is transferred;
//! a later overlapping write retires the affected field observations. A returned
//! record publishes the same field lengths on `result.field`, including when every match arm
//! builds that field from an equal window. An `ensures` bound that names a
//! parameter is that call argument's value, so `result.values.len == count`
//! arrives as the length the caller passed.
use crate::checks::unsigned;
use crate::values::scalar::structural_fields;
use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetHandle};
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use typed_trees::{machine::Machine, state::State};
use validation::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticHypothesis, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
};

mod collections;

#[derive(Clone)]
struct Binding {
    symbol: SymbolHandle,
    name: String,
    value: Option<ScopedArithmeticValue>,
}

#[derive(Clone, PartialEq, Eq)]
enum LengthSubject {
    Storage(SymbolHandle),
    Result(SymbolHandle),
    /// One captured slice field, identified by its complete declaration path.
    /// The owner is the current storage binding, not the source producer.
    Field {
        owner: SymbolHandle,
        path: Vec<SymbolHandle>,
    },
    /// One slice field of the value returned from this machine. The owner is
    /// the machine, and `result.field.len` reads this observation.
    ResultField {
        owner: SymbolHandle,
        path: Vec<SymbolHandle>,
    },
}

/// A captured descriptor observation, not the capacity of its backing owner.
#[derive(Clone)]
struct Length {
    subject: LengthSubject,
    name: String,
    stable: bool,
    type_reference: TypeReferenceHandle,
    value: Option<ScopedArithmeticValue>,
}

pub(crate) struct CurrentScalarValues<'program> {
    program: &'program dyn ::validation::CheckingInput,
    machine: &'program Machine,
    state: &'program State,
    statement_index: usize,
    initial: Vec<Binding>,
    current: Vec<Binding>,
    lengths: Vec<Length>,
    hypotheses: Vec<ScopedArithmeticHypothesis>,
}

impl<'program> CurrentScalarValues<'program> {
    pub(crate) fn at(
        program: &'program dyn ::validation::CheckingInput,
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
                let primitive = integer_carrier(program, parameter.type_reference)?;
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
        let lengths = program
            .state_parameters(state)
            .iter()
            .filter(|parameter| {
                validation::state_reference_parameter_binding_is_stable(
                    program,
                    machine,
                    state,
                    parameter.symbol,
                )
            })
            .filter_map(|parameter| {
                Some(Length {
                    subject: LengthSubject::Storage(parameter.symbol),
                    name: parameter.name.to_string(),
                    stable: true,
                    type_reference: parameter.type_reference,
                    value: Some(fixed_length(program, parameter.type_reference, || {
                        format!(
                            "state:{:?}:parameter:{:?}:length",
                            state.symbol, parameter.symbol
                        )
                    })?),
                })
            })
            .collect();
        let mut values = Self {
            program,
            machine,
            state,
            statement_index,
            current: initial.clone(),
            initial,
            lengths,
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
                if let Some(expression) = fact.as_expression() {
                    values.assume(program, machine, state, *expression, true);
                }
            }
        }
        let statements = program.statement_table.statements(state.statement_nodes);
        if statement_index > statements.len() {
            return None;
        }
        // Labels can skip or reorder lexical windows. Replay the already
        // admitted path, retaining source ordinals for captured observations;
        // a textual prefix could establish facts from unexecuted stores.
        let order = program
            .statement_table
            .admitted_execution_order(state.statement_nodes);
        let position = order.position(statement_index)?;
        for &ordinal in &order.indices()[..position] {
            let statement = &statements[ordinal];
            if validation::is_arm_pattern_marker(statement) {
                continue;
            }
            match statement {
                StatementNode::Transition(transition) => {
                    let TransitionGuardNode::When(guard) = transition.guard else {
                        return None;
                    };
                    values.invalidate_operand_writes(
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
                    values.capture_collection(
                        local.symbol,
                        local.name.as_str(),
                        local.type_reference,
                        local.initial_value,
                        ordinal,
                    );
                    values.capture_record_fields(
                        local.symbol,
                        &[],
                        local.name.as_str(),
                        local.initial_value,
                    );
                    if let Some(primitive) = integer_carrier(program, local.type_reference) {
                        // The executed initializer has one value even when its
                        // computation is unavailable to arithmetic reasoning.
                        // Keep that observation, not a replay of the initializer.
                        let value = value.or_else(|| {
                            Some(observed_scalar(state, local.symbol, ordinal, primitive))
                        });
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
                    let mut fields = values.captured_field_lengths(assignment.value, 0);
                    if let Some(value) = values.collection_length(assignment.value, 0)
                        && let Some(type_reference) = validation::expression_result_type_reference(
                            program,
                            machine,
                            state,
                            assignment.value,
                        )
                        && fixed_length(program, type_reference, String::new).is_some()
                    {
                        fields.push(collections::CapturedFieldLength {
                            path: Vec::new(),
                            name: String::new(),
                            type_reference,
                            value,
                        });
                    }
                    if let ExpressionNode::Name(name) =
                        program.expression_table.expression(assignment.target)
                        && name.symbol.is_valid()
                        && name.symbol == name.head_symbol
                        && let Some(binding) = values
                            .current
                            .iter_mut()
                            .find(|binding| binding.symbol == name.symbol)
                    {
                        binding.value = value.or_else(|| {
                            let reference = validation::expression_result_type_reference(
                                program,
                                machine,
                                state,
                                assignment.target,
                            )?;
                            let primitive = integer_carrier(program, reference)?;
                            Some(observed_scalar(state, name.symbol, ordinal, primitive))
                        });
                    } else if let ExpressionNode::Name(name) =
                        program.expression_table.expression(assignment.target)
                        && name.symbol == name.head_symbol
                        && let Some(length) = values
                            .lengths
                            .iter()
                            .find(|length| length.subject == LengthSubject::Storage(name.symbol))
                            .cloned()
                    {
                        values.capture_collection(
                            name.symbol,
                            &length.name,
                            length.type_reference,
                            assignment.value,
                            ordinal,
                        );
                    } else {
                        values.invalidate_operand_writes(
                            frames.map(|frames| frames.assignment_write_frame(machine, statement)),
                        );
                    }
                    if let Some((owner, path)) = field_storage_path(program, assignment.target) {
                        values.replace_record_fields(
                            owner,
                            &path,
                            &program.expression_table.display_name(assignment.target),
                            fields,
                        );
                    }
                }
                StatementNode::Call(call) => {
                    values.invalidate_operand_effects(program, machine, statement, frames);
                    values.invalidate_operand_writes(
                        frames.map(|frames| frames.may_write_frame(machine, call)),
                    );
                }
                StatementNode::Expression(_) => {
                    values.invalidate_operand_effects(program, machine, statement, frames)
                }
                // Control metadata has no store; instruction statements above
                // retain their ordinary effects. Assertions are obligations,
                // never premises introduced by this value transfer.
                StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}
                StatementNode::RootBinding(_) => return None,
            }
        }
        if target.is_valid() {
            let transition = statements.get(statement_index)?.as_transition()?;
            if transition.target != target && transition.continuation != target {
                return None;
            }
            if let TransitionGuardNode::When(guard) = transition.guard {
                values.invalidate_operand_writes(
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
                    values.invalidate_operand_writes(
                        frames.map(|frames| frames.expression_write_frame(machine, *argument)),
                    );
                }
            }
        }
        Some(values)
    }

    fn invalidate_operand_effects(
        &mut self,
        _program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        statement: &StatementNode,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) {
        // Complete existing operand frames include nested calls, rather than
        // assuming a syntactic assignment or local declaration is effect-free.
        if let Some(frames) = frames {
            self.invalidate_operand_writes(Some(
                frames.statement_value_write_frame(machine, statement),
            ));
        } else if statement_has_call(statement) {
            self.invalidate_operand_writes(None);
        }
    }

    /// Retire current value identities before observing operands that may run
    /// earlier calls. Unknown effects preserve no live scalar association.
    pub(crate) fn invalidate_operand_writes(&mut self, frame: Option<facts::NormalizedWriteFrame>) {
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
        for length in &mut self.lengths {
            if !length.stable
                && paths.as_ref().is_none_or(|paths| {
                    paths
                        .iter()
                        .any(|path| validation::frame_paths_overlap(path, &length.name))
                })
            {
                length.value = None;
            }
        }
    }

    fn assume(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        holds: bool,
    ) {
        self.assume_at_depth(program, machine, state, expression, holds, 0);
    }

    fn assume_at_depth(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        holds: bool,
        depth: usize,
    ) {
        if depth >= 64 || !program.expression_table.expression_is_valid(expression) {
            return;
        }
        if let Some(binary) = program.expression_table.binary_node(expression)
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
        if let Some(binary) = program.expression_table.binary_node(expression)
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
        if admitted(
            program,
            machine,
            state,
            expression,
            &self.current,
            &self.lengths,
            0,
        ) {
            if let Some(proposition) = scoped(
                machine,
                state,
                expression,
                &self.current,
                &self.lengths,
                program,
            ) {
                self.hypotheses
                    .push(ScopedArithmeticHypothesis { proposition, holds });
            }
        }
    }

    fn capture(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<ScopedArithmeticValue> {
        self.capture_at_depth(program, machine, state, expression, 0)
    }

    /// Capture an executed primitive's normal value, not a specification term.
    /// Exact and Trapping addition/subtraction/multiplication have their
    /// mathematical result on this continuation; a trap or an unproved exact
    /// overflow never reaches the use. Selection and the result carrier are
    /// checked independently. Wrapping and Saturating remain source terms
    /// with the engine's policy checks.
    fn capture_at_depth(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<ScopedArithmeticValue> {
        if depth >= 64 {
            return None;
        }
        if let ExpressionNode::Integer(literal) = program.expression_table.expression(expression)
            && let Some(value) = literal.value_i64()
        {
            return Some(ScopedArithmeticValue::Integer(
                numerics::bignum::BigInt::from_i64(value),
            ));
        }
        // An exact ensured result is that integer at every use. A proper
        // interval stays a term: `0 <= result < 26` does not select one end.
        if let Some(call) = program.expression_table.call_node(expression)
            && let Some((Some(low), Some(high))) =
                validation::normal_return_integer_bounds(program, call)
            && low == high
        {
            return Some(ScopedArithmeticValue::Integer(
                numerics::bignum::BigInt::from_i64(low),
            ));
        }
        if let ExpressionNode::Member(_) = program.expression_table.expression(expression) {
            if let Some(value) = field_occurrence(program, machine, state, expression) {
                return Some(value);
            }
            // A mutable parameter field is not frozen: a later store can change
            // it. Two reads still name one field, so a match arm can replace
            // that field with the pattern until a call in the same endpoint
            // might have written it.
            if let Some(value) = mutable_integer_field(program, machine, state, expression) {
                return Some(value);
            }
        }
        if let Some(binary) = program.expression_table.binary_node(expression)
            && matches!(
                binary.operator,
                BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
            )
            && validation::has_builtin_binary_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            )
            && let Some(reference) =
                validation::expression_result_type_reference(program, machine, state, expression)
            && integer_carrier(program, reference).is_some()
            && matches!(
                program.arithmetic_domain_for_type_reference(reference),
                ArithmeticDomain::Exact | ArithmeticDomain::Trapping
            )
        {
            return Some(ScopedArithmeticValue::IntegerOperation {
                operator: binary.operator,
                left: Box::new(self.capture_at_depth(
                    program,
                    machine,
                    state,
                    binary.left,
                    depth + 1,
                )?),
                right: Box::new(self.capture_at_depth(
                    program,
                    machine,
                    state,
                    binary.right,
                    depth + 1,
                )?),
            });
        }
        admitted(
            program,
            machine,
            state,
            expression,
            &self.current,
            &self.lengths,
            0,
        )
        .then(|| {
            scoped(
                machine,
                state,
                expression,
                &self.current,
                &self.lengths,
                program,
            )
        })
        .flatten()
        .map(ScopedArithmeticValue::Term)
    }

    /// Exact value of a runtime index at this occurrence. Its declaration
    /// symbol remains storage identity; the returned roster value captures
    /// assignments and unknown write effects at the selected program point.
    pub(crate) fn current_symbol(&self, symbol: SymbolHandle) -> Option<ScopedArithmeticValue> {
        self.current
            .iter()
            .find(|binding| binding.symbol == symbol)?
            .value
            .clone()
    }

    pub(crate) fn initial_symbol(&self, symbol: SymbolHandle) -> Option<ScopedArithmeticValue> {
        self.initial
            .iter()
            .find(|binding| binding.symbol == symbol)?
            .value
            .clone()
    }
    /// Read an authored expression under the captured arrival roster. Later
    /// scalar replacement cannot reinterpret this input as current storage.
    pub(crate) fn initial(
        &self,
        expression: ExpressionHandle,
    ) -> Option<ScopedArithmeticExpression> {
        admitted(
            self.program,
            self.machine,
            self.state,
            expression,
            &self.initial,
            &self.lengths,
            0,
        )
        .then(|| {
            scoped(
                self.machine,
                self.state,
                expression,
                &self.initial,
                &self.lengths,
                self.program,
            )
        })
        .flatten()
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
            &self.lengths,
            0,
        )
        .then(|| {
            scoped(
                self.machine,
                self.state,
                expression,
                &self.current,
                &self.lengths,
                self.program,
            )
        })
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
        let mut formals = Vec::with_capacity(parameters.len());
        let mut lengths = Vec::new();
        for (parameter, argument) in parameters.iter().zip(arguments) {
            let Some(primitive) = integer_carrier(self.program, parameter.type_reference) else {
                if let Some(value) = self.collection_length(*argument, 0)
                    && fixed_length(self.program, parameter.type_reference, String::new).is_some()
                    && (validation::state_reference_parameter_binding_is_stable(
                        self.program,
                        callee,
                        state,
                        parameter.symbol,
                    ) || matches!(
                        value,
                        ScopedArithmeticValue::Integer(_)
                            | ScopedArithmeticValue::BoundedInteger { .. }
                    ))
                {
                    // A closed length is an entry observation. A mutable slice
                    // parameter may later reseat its descriptor; the requires
                    // clause still sees the length the caller passed.
                    lengths.push(Length {
                        subject: LengthSubject::Storage(parameter.symbol),
                        name: parameter.name.to_string(),
                        stable: true,
                        type_reference: parameter.type_reference,
                        value: Some(value),
                    });
                }
                for field in self.captured_field_lengths(*argument, 0) {
                    lengths.push(Length {
                        subject: LengthSubject::Field {
                            owner: parameter.symbol,
                            path: field.path,
                        },
                        name: format!("{}.{}", parameter.name, field.name),
                        stable: true,
                        type_reference: field.type_reference,
                        value: Some(field.value),
                    });
                }
                continue;
            };
            formals.push(Binding {
                symbol: parameter.symbol,
                name: parameter.name.to_string(),
                value: Some(ScopedArithmeticValue::Atom {
                    identity: format!("callee:{:?}", parameter.symbol),
                    unsigned: unsigned(primitive),
                }),
            });
        }
        if !admitted(
            self.program,
            callee,
            state,
            expression,
            &formals,
            &lengths,
            0,
        ) {
            return None;
        }
        let mut goal = scoped(callee, state, expression, &formals, &lengths, self.program)?;
        for binding in &mut goal.bindings {
            // A callee field needs an actual-place substitution, including on
            // recursion. Its entry atom is never the caller's current field
            // merely because the declaration and formal symbol are equal.
            if let ScopedArithmeticBinder::Occurrence(occurrence) = binding.binder
                && length_occurrence(self.program, occurrence, &lengths).is_none()
                && literal_window_length(self.program, occurrence).is_none()
            {
                return None;
            }
            if let ScopedArithmeticBinder::Symbol(symbol) = binding.binder {
                // `scoped` selects the formal occurrences this goal actually
                // observes. An unrelated operand need not have arithmetic
                // meaning: it still evaluates and contributes its writes in
                // the caller's prefix before we bind any observed actual.
                let position = parameters
                    .iter()
                    .position(|parameter| parameter.symbol == symbol)?;
                let captured =
                    self.capture(self.program, self.machine, self.state, arguments[position])?;
                // The argument's captured identity remains mandatory. Bounds
                // from its exact caller observation add arrival knowledge
                // (including acyclic predecessors), never a destination
                // requirement or permission to revive an invalidated value.
                binding.value = if let Some((minimum, maximum)) =
                    validation::occurrence_integer_expression_bounds(
                        self.program,
                        self.machine.symbol,
                        self.state.symbol,
                        self.statement_index,
                        arguments[position],
                    ) {
                    ScopedArithmeticValue::BoundedInteger {
                        value: Box::new(captured),
                        minimum: Some(numerics::bignum::BigInt::from_i64(minimum)),
                        maximum: Some(numerics::bignum::BigInt::from_i64(maximum)),
                    }
                } else {
                    captured
                };
            }
        }
        Some(goal)
    }

    pub(crate) fn proves_values_relation(
        &self,
        program: &dyn ::validation::CheckingInput,
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
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        goal: &ScopedArithmeticExpression,
    ) -> bool {
        validation::scoped_arithmetic_implication(program, machine, &self.hypotheses, goal)
            == StrictArithmeticImplicationJudgment::Proven
    }
}

fn integer_carrier(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let primitive = program.primitive_type_reference(reference)?;
    // A stored payload has an integer denotation under every arithmetic policy.
    // This admits observation, not arithmetic with an erased policy.
    if !primitive.is_fixed_integer() {
        return None;
    }
    Some(primitive)
}
/// A collection type's length when no store or reseat can change it during
/// the activation: a literal fixed array's own length, or one unsigned unknown.
fn fixed_length(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
    identity: impl FnOnce() -> String,
) -> Option<ScopedArithmeticValue> {
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    let shared = *access == language_core::ReferenceAccess::Shared;
    let mut referee = *referee;
    // Fixed-array geometry is independent of qualification and loan access.
    // Dynamic slice observations retain their existing shared-qualified gate.
    let mut qualified = false;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(referee)
    {
        qualified = true;
        referee = *base_type;
    }
    match program.type_reference_table.type_reference(referee) {
        TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Literal(length),
            ..
        } => Some(ScopedArithmeticValue::Integer(
            numerics::bignum::BigInt::from_u64(u64::try_from(*length).ok()?),
        )),
        TypeReferenceNode::Slice { .. } if qualified && !shared => None,
        TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. } => {
            Some(ScopedArithmeticValue::Atom {
                identity: identity(),
                unsigned: true,
            })
        }
        _ => None,
    }
}

/// Exact length of `collection[start..end].len` when both bounds are closed
/// integer literals. A fixed array's own `.len` stays on the declared extent;
/// this is the same `end - start` a plain window comparison already uses.
fn literal_window_length(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<ScopedArithmeticValue> {
    let member = program.expression_table.member_node(expression)?;
    if member.case_variant.is_some() || member.member_symbol.is_valid() {
        return None;
    }
    let mut receiver = member.receiver;
    while let Some(borrow) = program.expression_table.borrow_expression(receiver) {
        receiver = borrow.target;
    }
    let indexed = program.expression_table.indexed_node(receiver)?;
    if !program.expression_table.indexed_on_range(indexed) {
        return None;
    }
    if validation::collection_operations::collection_measure_member(
        program,
        member,
        validation::collection_operations::MeasureReceiver::Window,
    ) != Some(language_semantics::declaration_selection::CollectionMeasure::Length)
    {
        return None;
    }
    let range = program.expression_table.range_node(indexed.index)?;
    let start = if range.start.is_valid() {
        closed_integer_literal(program, range.start)?
    } else {
        numerics::bignum::BigInt::zero()
    };
    if !range.end.is_valid() {
        return None;
    }
    let mut end = closed_integer_literal(program, range.end)?;
    if range.end_inclusive {
        end = end.add(&numerics::bignum::BigInt::from_u64(1));
    }
    if start > end {
        return None;
    }
    Some(ScopedArithmeticValue::Integer(end.sub(&start)))
}

fn closed_integer_literal(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<numerics::bignum::BigInt> {
    program
        .expression_table
        .integer_node(expression)?
        .value_bignum()
}

/// Static declaration fields retain their exact storage root and complete path.
fn field_storage_path(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<(SymbolHandle, Vec<SymbolHandle>)> {
    let place = validation::canonical_place_from_expression(program, expression)?;
    let owner = place.root.as_symbol()?;
    Some((owner, declaration_field_path(&place.segments)?))
}

fn declaration_field_path(segments: &[facts::PlaceSegment]) -> Option<Vec<SymbolHandle>> {
    segments
        .iter()
        .map(|segment| match segment {
            facts::PlaceSegment::Field { symbol } if symbol.is_valid() => Some(*symbol),
            _ => None,
        })
        .collect()
}

/// The fixed length a `collection.len` occurrence reads, if it reads one.
fn length_occurrence<'lengths>(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    lengths: &'lengths [Length],
) -> Option<&'lengths Length> {
    let member = program.expression_table.member_node(expression)?;
    if let Some(length) = lengths.iter().find(|length| {
        if length.value.is_none() {
            return false;
        }
        match &length.subject {
            LengthSubject::Field { owner, path } => field_storage_path(program, member.receiver)
                .is_some_and(|(actual_owner, actual_path)| {
                    actual_owner == *owner && actual_path == *path
                }),
            LengthSubject::ResultField { owner, path } => {
                validation::reserved_result_place(program, member.receiver).is_some_and(|place| {
                    place.machine_symbol == *owner
                        && declaration_field_path(&place.segments)
                            .is_some_and(|actual| actual == *path)
                })
            }
            _ => false,
        }
    }) {
        return (validation::collection_operations::collection_measure_member(
            program,
            member,
            validation::collection_operations::MeasureReceiver::Declared(length.type_reference),
        ) == Some(language_semantics::declaration_selection::CollectionMeasure::Length))
        .then_some(length);
    }
    let subject =
        if let Some((owner, _)) = validation::reserved_result_owner(program, member.receiver) {
            LengthSubject::Result(owner)
        } else {
            let name = program.expression_table.name_path_node(member.receiver)?;
            if !name.symbol.is_valid()
                || name.symbol != name.head_symbol
                || program
                    .expression_table
                    .name_path_members(name.members)
                    .len()
                    != 1
            {
                return None;
            }
            LengthSubject::Storage(name.symbol)
        };
    let length = lengths
        .iter()
        .find(|length| length.subject == subject && length.value.is_some())?;
    (validation::collection_operations::collection_measure_member(
        program,
        member,
        validation::collection_operations::MeasureReceiver::Declared(length.type_reference),
    ) == Some(language_semantics::declaration_selection::CollectionMeasure::Length))
    .then_some(length)
}

/// Frozen integer fields retain canonical root and complete declaration path.
/// Only stable shared parameter loans participate; owned or exclusive storage
/// needs an effect-sensitive value transfer before it can retain entry atoms.
fn frozen_field(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(TypeReferenceHandle, String)> {
    let parameters = program.state_parameters(state);
    let (index, path, selected_type) =
        structural_fields::structural_parameter_place(program, parameters, expression)?;
    if path.is_empty()
        || path.iter().any(|segment| {
            !matches!(
                segment,
                checked_trees::CheckedStructuralPredicatePathSegment::Field(_)
            )
        })
    {
        return None;
    }
    let parameter = parameters.get(usize::try_from(index).ok()?)?;
    let shell = validation::reference_shell(program, parameter.type_reference)?;
    if shell.access != language_core::ReferenceAccess::Shared
        || parameter.is_mutable
        || !validation::state_reference_parameter_binding_is_stable(
            program,
            machine,
            state,
            parameter.symbol,
        )
    {
        return None;
    }
    let (_, segments, _, frozen) =
        structural_fields::resolve_structural_parameter_path(program, parameters, index, &path)?;
    if !frozen {
        return None;
    }
    Some((
        selected_type,
        format!(
            "state:{:?}:parameter:{:?}:field:{segments:?}",
            state.symbol, parameter.symbol
        ),
    ))
}

/// Identity of an integer field of a parameter that `frozen_field` will not
/// freeze, including a mutable record. The identity is the field, not the
/// expression handle, so both reads in one arm compare equal.
fn mutable_integer_field(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<ScopedArithmeticValue> {
    let member = program.expression_table.member_node(expression)?;
    // Resolve the declaration, including prepared generic and case selectors.
    // Builtin measures have none: their values belong to the descriptor roster,
    // never an absent field identity shared by unrelated metadata.
    let field = validation::effective_member_symbol(program, member.receiver, member);
    if !field.is_valid() {
        return None;
    }
    let name = program.expression_table.name_path_node(member.receiver)?;
    if name.symbol != name.head_symbol
        || program
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return None;
    }
    let reference =
        validation::expression_result_type_reference(program, machine, state, expression)?;
    let primitive = integer_carrier(program, reference)?;
    Some(ScopedArithmeticValue::Atom {
        identity: format!(
            "state:{:?}:parameter:{:?}:member:{:?}",
            state.symbol, name.symbol, field
        ),
        unsigned: unsigned(primitive),
    })
}

fn field_collection_length(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<ScopedArithmeticValue> {
    let (reference, identity) = frozen_field(program, machine, state, expression)?;
    fixed_length(program, reference, || format!("{identity}:length"))
}

fn field_occurrence(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<ScopedArithmeticValue> {
    if let Some(member) = program.expression_table.member_node(expression)
        && let Some((reference, _)) = frozen_field(program, machine, state, member.receiver)
        && validation::collection_operations::collection_measure_member(
            program,
            member,
            validation::collection_operations::MeasureReceiver::Declared(reference),
        ) == Some(language_semantics::declaration_selection::CollectionMeasure::Length)
    {
        return field_collection_length(program, machine, state, member.receiver);
    }
    let (reference, identity) = frozen_field(program, machine, state, expression)?;
    let primitive = integer_carrier(program, reference)?;
    let value = ScopedArithmeticValue::Atom {
        identity,
        unsigned: unsigned(primitive),
    };
    // Standing declaration bounds constrain this same frozen field identity.
    // They neither replay an initializer nor import a caller's changing facts.
    Some(
        match validation::immutable_integer_expression_bounds(program, machine, state, expression) {
            Some((minimum, maximum)) => ScopedArithmeticValue::BoundedInteger {
                value: Box::new(value),
                minimum: Some(numerics::bignum::BigInt::from_i64(minimum)),
                maximum: Some(numerics::bignum::BigInt::from_i64(maximum)),
            },
            None => value,
        },
    )
}

/// Identity belongs to this executed store on the current state activation.
/// Distinct stores never share an unknown merely because they have the same
/// destination or initializer; subsequent effects still retire the binding.
fn observed_scalar(
    state: &State,
    symbol: SymbolHandle,
    statement: usize,
    primitive: PrimitiveType,
) -> ScopedArithmeticValue {
    ScopedArithmeticValue::Atom {
        identity: format!(
            "state:{:?}:store:{statement}:scalar:{symbol:?}",
            state.symbol
        ),
        unsigned: unsigned(primitive),
    }
}

fn admitted(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    bindings: &[Binding],
    lengths: &[Length],
    depth: usize,
) -> bool {
    if depth >= 64 || !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) => true,
        ExpressionNode::Call(_) => validation::integer_embedding_argument(program, expression)
            .is_some_and(|(_, argument)| {
                admitted(
                    program,
                    machine,
                    state,
                    argument,
                    bindings,
                    lengths,
                    depth + 1,
                )
            }),
        ExpressionNode::Member(_) => {
            length_occurrence(program, expression, lengths).is_some()
                || field_occurrence(program, machine, state, expression).is_some()
                || literal_window_length(program, expression).is_some()
        }
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
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
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
            ) && admitted(
                program,
                machine,
                state,
                binary.left,
                bindings,
                lengths,
                depth + 1,
            ) && admitted(
                program,
                machine,
                state,
                binary.right,
                bindings,
                lengths,
                depth + 1,
            )
        }
        _ => false,
    }
}

fn scoped(
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    bindings: &[Binding],
    lengths: &[Length],
    program: &dyn ::validation::CheckingInput,
) -> Option<ScopedArithmeticExpression> {
    let mut symbols = Vec::new();
    let mut projections = Vec::new();
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
            ExpressionNode::Call(_) => {
                let (_, argument) = validation::integer_embedding_argument(program, expression)?;
                pending.push((argument, depth + 1));
            }
            ExpressionNode::Member(_) => {
                let value = length_occurrence(program, expression, lengths)
                    .and_then(|length| length.value.clone())
                    .or_else(|| field_occurrence(program, machine, state, expression))
                    .or_else(|| literal_window_length(program, expression))?;
                projections.push(ScopedArithmeticBinding {
                    binder: ScopedArithmeticBinder::Occurrence(expression),
                    value,
                });
            }
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
    selected.extend(projections);
    Some(ScopedArithmeticExpression {
        owner: state.symbol,
        expression,
        bindings: selected,
    })
}
fn charge(value: &ScopedArithmeticValue, budget: &mut usize, depth: usize) -> Option<()> {
    *budget = budget.checked_sub(1)?;
    if depth >= 64 {
        return None;
    }
    match value {
        ScopedArithmeticValue::Term(term)
        | ScopedArithmeticValue::CollectionLength(term)
        | ScopedArithmeticValue::AdmittedIntegerRemainder(term) => {
            for binding in &term.bindings {
                charge(&binding.value, budget, depth + 1)?;
            }
        }
        ScopedArithmeticValue::BoundedInteger { value, .. } => {
            charge(value, budget, depth + 1)?;
        }
        ScopedArithmeticValue::IntegerOperation { left, right, .. } => {
            charge(left, budget, depth + 1)?;
            charge(right, budget, depth + 1)?;
        }
        ScopedArithmeticValue::Atom { .. } | ScopedArithmeticValue::Integer(_) => {}
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

/// The current descriptor observation's proven upper bound, after every
/// preceding store and call has retired or replaced its captured facts.
pub(crate) fn byte_length_bound(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<u64> {
    let machine = program.machine_by_own_or_state_symbol(state.symbol)?;
    CurrentScalarValues::at(
        program,
        machine,
        state,
        statement_index,
        TransitionTargetHandle::invalid(),
        None,
    )?
    .byte_length_bound(expression)
}

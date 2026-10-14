//! Current scalar values and descriptor measures across statement transfers.
//!
//! A hypothesis captures its input roster when it is established. Assignment
//! captures the RHS under that roster before replacing its destination; later
//! reads never reinterpret an earlier name as the replacement. This is private
//! proof construction scratch, not an invocation-memory snapshot or a retained
//! certificate. Arrivals, exits and ranks share this transfer. Unknown effects
//! retire current bindings; they cannot revive an entry fact about new storage.
//!
//! A collection loan's length is one value for the whole activation when the
//! binding cannot be reseated and the loan cannot resize its referent: a slice
//! or plain fixed array behind any reference, or anything behind a shared one.
//! Each `bytes.len` occurrence binds as an exact projection of that value, so a
//! `requires index < bytes.len` and a guard over the same loan relate. A
//! mutable loan of a qualified fixed byte array is a bounded owner whose live
//! length moves with its stores, and has no such value. Local descriptors use
//! the same replacement and effect retirement as scalar bindings; their
//! captured measures survive endpoint stores, never descriptor replacement.
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum LengthSubject {
    Storage(SymbolHandle),
    Result(SymbolHandle),
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
        for statement in &statements[..statement_index] {
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
                    );
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
                        );
                    } else {
                        values.invalidate_operand_writes(
                            frames.map(|frames| frames.assignment_write_frame(machine, statement)),
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
                _ => return None,
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
            let Some(primitive) = exact_integer(self.program, parameter.type_reference) else {
                if validation::state_reference_parameter_binding_is_stable(
                    self.program,
                    callee,
                    state,
                    parameter.symbol,
                ) && let Some(value) = self.collection_length(*argument, 0)
                    && fixed_length(self.program, parameter.type_reference, String::new).is_some()
                {
                    lengths.push(Length {
                        subject: LengthSubject::Storage(parameter.symbol),
                        name: parameter.name.to_string(),
                        stable: true,
                        type_reference: parameter.type_reference,
                        value: Some(value),
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
                binding.value = ScopedArithmeticValue::Term(self.current(arguments[position])?);
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

fn exact_integer(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let primitive = program.primitive_type_reference(reference)?;
    if !primitive.is_fixed_integer()
        || program.arithmetic_domain_for_type_reference(reference) != ArithmeticDomain::Exact
    {
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
    // A qualified fixed byte array is a bounded owner: its length is the live
    // byte count, at most the declared capacity. Only a shared loan freezes it.
    let mut qualified = false;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(referee)
    {
        if !shared {
            return None;
        }
        qualified = true;
        referee = *base_type;
    }
    match program.type_reference_table.type_reference(referee) {
        TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Literal(length),
            ..
        } if !qualified => Some(ScopedArithmeticValue::Integer(
            numerics::bignum::BigInt::from_u64(u64::try_from(*length).ok()?),
        )),
        TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. } => {
            Some(ScopedArithmeticValue::Atom {
                identity: identity(),
                unsigned: true,
            })
        }
        _ => None,
    }
}

/// The fixed length a `collection.len` occurrence reads, if it reads one.
fn length_occurrence<'lengths>(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    lengths: &'lengths [Length],
) -> Option<&'lengths Length> {
    let member = program.expression_table.member_node(expression)?;
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
fn field_occurrence(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<ScopedArithmeticValue> {
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
    let primitive = exact_integer(program, selected_type)?;
    Some(ScopedArithmeticValue::Atom {
        identity: format!(
            "state:{:?}:parameter:{:?}:field:{segments:?}",
            state.symbol, parameter.symbol
        ),
        unsigned: unsigned(primitive),
    })
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
        ExpressionNode::Member(_) => {
            length_occurrence(program, expression, lengths).is_some()
                || field_occurrence(program, machine, state, expression).is_some()
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
            ExpressionNode::Member(_) => {
                let value = length_occurrence(program, expression, lengths)
                    .and_then(|length| length.value.clone())
                    .or_else(|| field_occurrence(program, machine, state, expression))?;
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

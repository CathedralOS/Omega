//! Fixed-integer result guarantees from the complete ordinary state graph.
//!
//! Each explicit normal return supplies a carrier interval, optionally narrowed
//! by its own current guard. Internal state transfers supply no hypotheses:
//! every state's returns must qualify independently. Immutable captures retain
//! resolved declaration identity and an uninterrupted storage interval; array
//! initializers never supply the value of an indexed read.

use super::arithmetic_judgment::{Engine, Judgment, Polynomial};
use super::{ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticValue};
use crate::value_custody::places::{
    exact_data_member_field, exact_self_field, unwrapped_type_reference,
};
use facts::IntegerRange;
use numerics::bignum::BigInt;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

#[cfg(test)]
mod tests;

pub(super) fn infer(program: &TypedTrees, machine: &Machine) -> Option<IntegerRange> {
    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !super::entailment_covers_all_exits(program, machine)
    {
        return None;
    }
    let states = program.machine_states(machine);
    let primitive = integer_carrier(program, states.first()?.return_type)?;
    let frames = crate::CallFrameResolver::new(program)?;
    let mut combined: Option<IntegerRange> = None;
    for state in states {
        if integer_carrier(program, state.return_type) != Some(primitive) {
            return None;
        }
        let reader = ReturnReader {
            program,
            machine,
            state,
            frames: &frames,
            primitive,
        };
        let statements = program.statement_table.statements(state.statement_nodes);
        // Guard coverage alone says nothing about an implicit normal exit.
        // Empty bodies and a final declaration/store cannot supply a value.
        if !matches!(
            statements.last(),
            Some(StatementNode::Expression(_) | StatementNode::Transition(_))
        ) {
            return None;
        }
        let mut guards = Vec::new();
        for (statement, node) in statements.iter().enumerate() {
            match node {
                StatementNode::Expression(expression) if statement + 1 == statements.len() => {
                    join(
                        &mut combined,
                        reader.value(*expression, statement, &guards)?,
                    );
                }
                StatementNode::Transition(transition) => {
                    let guard = match transition.guard {
                        TransitionGuardNode::When(expression) => {
                            reader
                                .stable(expression, statement, 0)
                                .then_some(expression)?;
                            Some(expression)
                        }
                        TransitionGuardNode::Always => None,
                    };
                    for (target, holds) in
                        [(transition.target, true), (transition.continuation, false)]
                    {
                        if !target.is_valid() {
                            continue;
                        }
                        match program.statement_table.transition_target(target) {
                            TransitionTargetNode::Value(expression) => {
                                if let Some(guard) = guard {
                                    guards.push((guard, holds, statement));
                                }
                                join(
                                    &mut combined,
                                    reader.value(*expression, statement, &guards)?,
                                );
                                if guard.is_some() {
                                    guards.pop();
                                }
                            }
                            TransitionTargetNode::Named {
                                path,
                                arguments,
                                static_machine_parameter,
                                evidence_arguments,
                                ..
                            } => {
                                if static_machine_parameter.is_valid()
                                    || !evidence_arguments.is_empty()
                                    || !states.iter().any(|target| target.symbol == path.symbol)
                                    || !program
                                        .statement_table
                                        .expression_handles(*arguments)
                                        .iter()
                                        .all(|argument| reader.stable(*argument, statement, 0))
                                {
                                    return None;
                                }
                            }
                            // An explicit self-state edge is a loop, not an
                            // induction hypothesis or a returned value.
                            TransitionTargetNode::SelfTarget => {}
                            TransitionTargetNode::Terminal => return None,
                        }
                    }
                    // Reaching a later statement means this ordered arm did
                    // not take its target. No returned-value hypothesis is
                    // imported from that target or from another state.
                    if !transition.continuation.is_valid()
                        && let Some(guard) = guard
                    {
                        guards.push((guard, false, statement));
                    }
                }
                StatementNode::LocalData(local) => {
                    if !reader.stable(local.initial_value, statement, 0) {
                        return None;
                    }
                }
                StatementNode::Expression(expression) => {
                    if !reader.stable(*expression, statement, 0) {
                        return None;
                    }
                }
                // Existing inductive/proof routes retain other body operations.
                // A store or call cannot silently preserve a prior capture.
                _ => return None,
            }
        }
    }
    combined
}

fn join(combined: &mut Option<IntegerRange>, bounds: IntegerRange) {
    *combined = Some(match combined.take() {
        Some(previous) => IntegerRange {
            minimum: previous.minimum.min(bounds.minimum),
            maximum: previous.maximum.max(bounds.maximum),
        },
        None => bounds,
    });
}

pub(super) fn proves(
    program: &TypedTrees,
    machine: &Machine,
    goal: ExpressionHandle,
    bounds: &IntegerRange,
) -> bool {
    let Some(entry) = program.machine_states(machine).first() else {
        return false;
    };
    if !result_goal(program, machine, goal, 0)
        || !crate::has_builtin_bound_expression_meaning(program, machine, Some(entry), goal)
    {
        return false;
    }
    let mut engine = Engine::strict_with_symbol_bindings(program, machine, &[]);
    let identity = "ordinary-return".to_owned();
    if !engine.install_scoped_bindings(&[ScopedArithmeticBinding {
        binder: ScopedArithmeticBinder::Result,
        value: ScopedArithmeticValue::Atom {
            identity: identity.clone(),
            unsigned: false,
        },
    }]) {
        return false;
    }
    let result = Polynomial::atom(identity);
    if !engine.install_hypotheses(vec![
        (
            BinaryOperator::LessOrEqual,
            Polynomial::constant(bounds.minimum.clone()),
            result.clone(),
        ),
        (
            BinaryOperator::LessOrEqual,
            result,
            Polynomial::constant(bounds.maximum.clone()),
        ),
    ]) {
        return false;
    }
    matches!(engine.judge(goal), Judgment::Proven)
}

fn result_goal(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
    depth: usize,
) -> bool {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => true,
        ExpressionNode::Name(_) => crate::reserved_result_owner(program, expression)
            .is_some_and(|(owner, _)| owner == machine.symbol),
        ExpressionNode::Binary(binary) => {
            matches!(
                binary.operator,
                BinaryOperator::And
                    | BinaryOperator::Or
                    | BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
            ) && result_goal(program, machine, binary.left, depth + 1)
                && result_goal(program, machine, binary.right, depth + 1)
        }
        // Strict arithmetic can also consume callee result guarantees. This
        // body judgment supplies no such assumptions or named-parameter facts.
        _ => false,
    }
}

struct ReturnReader<'program, 'frames> {
    program: &'program TypedTrees,
    machine: &'program Machine,
    state: &'program State,
    frames: &'frames crate::CallFrameResolver<'program>,
    primitive: PrimitiveType,
}

impl ReturnReader<'_, '_> {
    fn value(
        &self,
        expression: ExpressionHandle,
        statement: usize,
        guards: &[(ExpressionHandle, bool, usize)],
    ) -> Option<IntegerRange> {
        if !self.stable(expression, statement, 0) {
            return None;
        }
        let (returned, before) = self.capture(expression, statement, statement, 0)?;
        if let ExpressionNode::Integer(literal) = self.program.expression_table.expression(returned)
        {
            let value = literal.value_bignum()?;
            typed_trees::closed_numeric::land_integer(&value, self.primitive)?;
            return Some(IntegerRange {
                minimum: value.clone(),
                maximum: value,
            });
        }
        if integer_carrier(self.program, self.place_type(returned, before, 0)?)
            != Some(self.primitive)
        {
            return None;
        }
        let mut bounds = carrier_bounds(self.primitive)?;
        for &(guard, holds, before) in guards {
            self.refine(guard, holds, before, statement, returned, &mut bounds, 0);
        }
        (bounds.minimum <= bounds.maximum).then_some(bounds)
    }

    /// Follow only exact immutable primitive captures. A decreasing definition
    /// coordinate prevents forward/cyclic substitution; storage is never read
    /// from an aggregate initializer to invent an element value.
    fn capture(
        &self,
        mut expression: ExpressionHandle,
        mut before: usize,
        exit: usize,
        mut depth: usize,
    ) -> Option<(ExpressionHandle, usize)> {
        loop {
            if depth >= 128 {
                return None;
            }
            depth += 1;
            let ExpressionNode::Name(path) = self.program.expression_table.expression(expression)
            else {
                break;
            };
            let statements = self
                .program
                .statement_table
                .statements(self.state.statement_nodes);
            let mut locals = statements
                .iter()
                .enumerate()
                .filter_map(|(position, node)| {
                    let StatementNode::LocalData(local) = node else {
                        return None;
                    };
                    (local.symbol == path.symbol).then_some((position, local))
                });
            let Some((definition, local)) = locals.next() else {
                break;
            };
            if locals.next().is_some()
                || definition >= before
                || local.is_mutable
                || path.head_symbol != local.symbol
                || !local.symbol.is_valid()
                || self
                    .program
                    .primitive_type_reference(local.type_reference)
                    .is_none()
                || !self.fence(definition, exit)
            {
                return None;
            }
            expression = local.initial_value;
            before = definition;
        }
        Some((expression, before))
    }

    fn fence(&self, first: usize, exit: usize) -> bool {
        let statements = self
            .program
            .statement_table
            .statements(self.state.statement_nodes);
        let Some(interval) = statements.get(first..exit) else {
            return false;
        };
        interval.iter().enumerate().all(|(offset, node)| {
            self.frames
                .statement_value_write_frame(self.machine, node)
                .into_complete_paths()
                .is_some_and(|paths| paths.is_empty())
                && match node {
                    StatementNode::LocalData(local) => {
                        self.stable(local.initial_value, first + offset, 0)
                    }
                    StatementNode::Expression(expression) => {
                        self.stable(*expression, first + offset, 0)
                    }
                    StatementNode::Transition(transition)
                        if !transition.continuation.is_valid() =>
                    {
                        matches!(transition.guard, TransitionGuardNode::When(guard)
                            if self.stable(guard, first + offset, 0))
                    }
                    _ => false,
                }
        })
    }

    fn refine(
        &self,
        condition: ExpressionHandle,
        holds: bool,
        before: usize,
        exit: usize,
        returned: ExpressionHandle,
        bounds: &mut IntegerRange,
        depth: usize,
    ) {
        if depth >= 128 {
            return;
        }
        let Some((condition, before)) = self.capture(condition, before, exit, depth) else {
            return;
        };
        if !self.stable(condition, before, 0) {
            return;
        }
        let ExpressionNode::Binary(binary) = self.program.expression_table.expression(condition)
        else {
            return;
        };
        if (binary.operator == BinaryOperator::And && holds)
            || (binary.operator == BinaryOperator::Or && !holds)
        {
            self.refine(
                binary.left,
                holds,
                before,
                exit,
                returned,
                bounds,
                depth + 1,
            );
            self.refine(
                binary.right,
                holds,
                before,
                exit,
                returned,
                bounds,
                depth + 1,
            );
            return;
        }
        if matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::NotEqual
        ) {
            for (boolean, predicate) in [(binary.left, binary.right), (binary.right, binary.left)] {
                if let ExpressionNode::Boolean(value) =
                    self.program.expression_table.expression(boolean)
                {
                    self.refine(
                        predicate,
                        holds == (*value == (binary.operator == BinaryOperator::Equal)),
                        before,
                        exit,
                        returned,
                        bounds,
                        depth + 1,
                    );
                    return;
                }
            }
        }
        let Some((left, _)) = self.capture(binary.left, before, exit, depth + 1) else {
            return;
        };
        let Some((right, _)) = self.capture(binary.right, before, exit, depth + 1) else {
            return;
        };
        let (endpoint, on_left) = if self
            .program
            .expression_table
            .expressions_structurally_equal(left, returned)
        {
            (right, true)
        } else if self
            .program
            .expression_table
            .expressions_structurally_equal(right, returned)
        {
            (left, false)
        } else {
            return;
        };
        let ExpressionNode::Integer(literal) = self.program.expression_table.expression(endpoint)
        else {
            return;
        };
        let Some(value) = literal.value_bignum() else {
            return;
        };
        if typed_trees::closed_numeric::land_integer(&value, self.primitive).is_none() {
            return;
        }
        let operator = if holds {
            binary.operator
        } else {
            let Some(operator) = super::inductive_judgment::negated_comparison(binary.operator)
            else {
                return;
            };
            operator
        };
        let one = BigInt::from_i64(1);
        match (operator, on_left) {
            (BinaryOperator::Equal, _) => {
                bounds.minimum = bounds.minimum.clone().max(value.clone());
                bounds.maximum = bounds.maximum.clone().min(value);
            }
            (BinaryOperator::Less, true) | (BinaryOperator::Greater, false) => {
                bounds.maximum = bounds.maximum.clone().min(value.sub(&one))
            }
            (BinaryOperator::LessOrEqual, true) | (BinaryOperator::GreaterOrEqual, false) => {
                bounds.maximum = bounds.maximum.clone().min(value)
            }
            (BinaryOperator::Greater, true) | (BinaryOperator::Less, false) => {
                bounds.minimum = bounds.minimum.clone().max(value.add(&one))
            }
            (BinaryOperator::GreaterOrEqual, true) | (BinaryOperator::LessOrEqual, false) => {
                bounds.minimum = bounds.minimum.clone().max(value)
            }
            _ => {}
        }
    }

    fn stable(&self, expression: ExpressionHandle, before: usize, depth: usize) -> bool {
        if depth >= 128
            || !self
                .program
                .expression_table
                .expression_is_valid(expression)
            || !crate::has_builtin_bound_expression_meaning(
                self.program,
                self.machine,
                Some(self.state),
                expression,
            )
        {
            return false;
        }
        let children = match self.program.expression_table.expression(expression) {
            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => true,
            ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
                self.place_type(expression, before, depth + 1).is_some()
            }
            ExpressionNode::Binary(binary) => {
                self.stable(binary.left, before, depth + 1)
                    && self.stable(binary.right, before, depth + 1)
            }
            ExpressionNode::ArrayLiteral(elements) => self
                .program
                .expression_table
                .expression_handles(*elements)
                .iter()
                .all(|element| self.stable(*element, before, depth + 1)),
            // A readonly call can be nondeterministic. An empty write frame
            // alone never licenses its repeated evaluation as one value.
            _ => false,
        };
        children
            && self
                .frames
                .expression_write_frame(self.machine, expression)
                .into_complete_paths()
                .is_some_and(|paths| paths.is_empty())
    }

    fn place_type(
        &self,
        expression: ExpressionHandle,
        before: usize,
        depth: usize,
    ) -> Option<TypeReferenceHandle> {
        if depth >= 128 {
            return None;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                if !path.symbol.is_valid()
                    || path.head_symbol != path.symbol
                    || self
                        .program
                        .expression_table
                        .name_path_members(path.members)
                        .len()
                        != 1
                {
                    return None;
                }
                let mut parameters = self
                    .program
                    .state_parameters(self.state)
                    .iter()
                    .filter(|parameter| parameter.symbol == path.symbol);
                if let Some(parameter) = parameters.next() {
                    return parameters
                        .next()
                        .is_none()
                        .then_some(parameter.type_reference);
                }
                let statements = self
                    .program
                    .statement_table
                    .statements(self.state.statement_nodes);
                let mut locals = statements.get(..before)?.iter().filter_map(|node| {
                    let StatementNode::LocalData(local) = node else {
                        return None;
                    };
                    (local.symbol == path.symbol).then_some(local.type_reference)
                });
                let reference = locals.next()?;
                locals.next().is_none().then_some(reference)
            }
            ExpressionNode::Member(member) => {
                // Attached self expressions retain the machine's symbol, not
                // the parameter's. Rejoin the existing exact field identity
                // and the current state's actual receiver declaration.
                if let Some(field) = exact_self_field(self.program, self.machine, expression) {
                    let mut receivers = self
                        .program
                        .state_parameters(self.state)
                        .iter()
                        .filter(|parameter| parameter.is_self);
                    let receiver = receivers.next()?;
                    if receivers.next().is_some() {
                        return None;
                    }
                    let reference =
                        unwrapped_type_reference(self.program, receiver.type_reference)?;
                    let owner = match self.program.type_reference_table.type_reference(reference) {
                        TypeReferenceNode::Named { symbol, .. } => *symbol,
                        TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
                        _ => return None,
                    };
                    // Pristine receiver `Self` is bound to the machine;
                    // realized nominal receivers bind the attached data.
                    return (owner == self.machine.symbol
                        || owner == self.machine.attached_data_symbol)
                        .then_some(field.type_reference);
                }
                let receiver = unwrapped_type_reference(
                    self.program,
                    self.place_type(member.receiver, before, depth + 1)?,
                )?;
                let owner = match self.program.type_reference_table.type_reference(receiver) {
                    TypeReferenceNode::Named { symbol, .. } => *symbol,
                    TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
                    _ => return None,
                };
                let data = crate::program_scope::data_definitions::data_definition_by_symbol(
                    self.program,
                    owner,
                )?;
                Some(
                    exact_data_member_field(
                        self.program,
                        data,
                        member.member_symbol,
                        member.member.as_str(),
                        member.case_variant.as_ref().map(|case| case.as_str()),
                    )?
                    .type_reference,
                )
            }
            ExpressionNode::Indexed(indexed) => {
                let collection = self.place_type(indexed.collection, before, depth + 1)?;
                let index_type = match self.program.expression_table.expression(indexed.index) {
                    ExpressionNode::Integer(_) => None,
                    ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
                        let reference = self.place_type(indexed.index, before, depth + 1)?;
                        integer_carrier(self.program, reference)?;
                        Some(reference)
                    }
                    _ => return None,
                };
                // Pristine typed trees retain a checked-operator selection
                // request, before stage 04 publishes its intrinsic outcome.
                // Rejoin that one exact occurrence and independently resolve
                // both ordinary/trait and implicit-borrow indexed meanings.
                let mut occurrences = self
                    .program
                    .expression_table
                    .authored_selection_occurrences(expression);
                let occurrence = occurrences.next()?;
                if occurrences.next().is_some()
                    || self
                        .program
                        .authored_declaration_selections()
                        .get(occurrence)?
                        .kind()
                        != typed_trees::AuthoredDeclarationSelectionKind::Operator
                    || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                        self.program,
                        self.machine.symbol,
                        expression,
                        language_core::OperatorSpelling::Index,
                        &[Some(collection), index_type],
                    )
                    || !typed_trees::operator::resolve_indexed_spelling_for_operands(
                        self.program,
                        language_core::OperatorSpelling::Index,
                        &[Some(collection), index_type],
                        self.program.expression_table.source_span(expression),
                    )
                    .is_empty()
                {
                    return None;
                }
                let collection = unwrapped_type_reference(self.program, collection)?;
                match self.program.type_reference_table.type_reference(collection) {
                    TypeReferenceNode::FixedArray { element_type, .. }
                    | TypeReferenceNode::Slice { element_type } => Some(*element_type),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

fn integer_carrier(program: &TypedTrees, reference: TypeReferenceHandle) -> Option<PrimitiveType> {
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    program.symbols.builtin_type_atom(*symbol)?;
    let primitive = program.primitive_type_reference(reference)?;
    carrier_bounds(primitive)?;
    (program.arithmetic_domain_for_type_reference(reference)
        == numerics::arithmetic::ArithmeticDomain::Exact)
        .then_some(primitive)
}

fn carrier_bounds(primitive: PrimitiveType) -> Option<IntegerRange> {
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
    ) {
        return None;
    }
    let bits = primitive.scalar_byte_size()? * 8;
    let (minimum, maximum) = if primitive.is_signed_integer() {
        (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1)
    } else {
        (0, (1i128 << bits) - 1)
    };
    Some(IntegerRange {
        minimum: BigInt::from_i128(minimum),
        maximum: BigInt::from_i128(maximum),
    })
}

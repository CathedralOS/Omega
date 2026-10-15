//! A taken guard may read a scalar copy made before the guard. Normalization
//! and authored locals have the same retained store evidence. Relate that copy
//! to its source only while both the destination and every source/selector observation
//! survive through this exact invocation. This does not replay an initializer
//! or a callee, and a write followed by restoration still breaks the relation.
use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

pub(super) fn conjunct_matches(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    guard: ExpressionHandle,
    required: ExpressionHandle,
) -> bool {
    let Some(parameters) = validation::call_target_parameters(program, call.target_symbol) else {
        return false;
    };
    let Some(site) = validation::find_call_site(
        program,
        state.machine_symbol,
        state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    ) else {
        return false;
    };
    let arguments = validation::call_site_argument_expressions(program, &site);
    let aliases = stable_copies(program, facts, state, call);
    Comparison {
        program,
        facts,
        state,
        call,
        parameters,
        arguments,
        aliases: &aliases,
    }
    .conjunct(guard, required)
}

fn stable_copies(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
) -> Vec<(validation::CanonicalPlace, ExpressionHandle)> {
    let Some(invocation) = facts
        .flow
        .control
        .operands
        .span(call.operands)
        .and_then(|values| values.last())
    else {
        return Vec::new();
    };
    let Some(statements) = facts.flow.control.statements.span(state.statements) else {
        return Vec::new();
    };
    let mut aliases = Vec::new();
    let mut seen = Vec::new();
    for statement in statements
        .iter()
        .rev()
        .filter(|statement| statement.statement_index < call.statement_index)
    {
        if !facts.semantic.places.is_valid(statement.stored_destination)
            || !facts.flow.control.operands.is_valid(statement.stored_value)
        {
            continue;
        }
        let Some(destination) = crate::flow::canonical_place_from_semantic_place(
            program,
            &facts.semantic,
            facts.semantic.places.get(statement.stored_destination),
        ) else {
            continue;
        };
        if seen
            .iter()
            .any(|prior| same_place(program, prior, &destination))
        {
            continue;
        }
        seen.push(destination.clone());
        let stored = facts.flow.control.operands.get(statement.stored_value);
        if program
            .primitive_type_reference(stored.type_reference)
            .is_none()
            || !super::captured_boolean::computed_values_have_source_identity(
                program,
                facts,
                stored.expression,
            )
        {
            continue;
        }
        // The defining store changes the destination itself. Only subsequent writes
        // participate in destination-currentness; source-currentness below starts at
        // the RHS's retained completion before that defining store.
        if !statements
            .iter()
            .filter(|later| {
                later.statement_index > statement.statement_index
                    && later.statement_index < call.statement_index
            })
            .all(|later| {
                crate::flow::entry_origin_is_stable(
                    program,
                    &facts.flow.control,
                    later.completed_content_writes,
                    later.completed_content_unknown_write,
                    &destination,
                )
            })
            || !crate::flow::operand_entry_origin_is_stable(
                program,
                &facts.flow,
                invocation,
                &destination,
            )
        {
            continue;
        }
        let mut reads = Vec::new();
        crate::facts::contract_occurrences::append_expression_occurrences(
            program,
            stored.expression,
            &mut reads,
        );
        let mut selectors_preserve_identity = true;
        for segment in &destination.segments {
            if let facts::PlaceSegment::Index { expression } = segment {
                selectors_preserve_identity &=
                    super::captured_boolean::computed_values_have_source_identity(
                        program,
                        facts,
                        *expression,
                    );
                crate::facts::contract_occurrences::append_expression_occurrences(
                    program,
                    *expression,
                    &mut reads,
                );
            }
        }
        if selectors_preserve_identity
            && reads.iter().all(|read| {
                validation::canonical_place_from_expression_in_state(
                    program,
                    state.state_symbol,
                    stored.statement_index,
                    *read,
                )
                .is_some_and(|place| {
                    crate::flow::operand_observations_agree(
                        program,
                        &facts.flow,
                        stored,
                        invocation,
                        &place,
                    )
                })
            })
        {
            aliases.push((destination, stored.expression));
        }
    }
    aliases
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Namespace {
    Caller,
    Callee,
}

struct Comparison<'a> {
    program: &'a dyn ::validation::CheckingInput,
    facts: &'a CheckFacts,
    state: &'a FlowStateFact,
    call: &'a FlowCallFact,
    parameters: &'a [typed_trees::signature::StateParameter],
    arguments: &'a [ExpressionHandle],
    aliases: &'a [(validation::CanonicalPlace, ExpressionHandle)],
}

impl Comparison<'_> {
    fn conjunct(&self, guard: ExpressionHandle, required: ExpressionHandle) -> bool {
        // Decomposing a selected Boolean operation uses its builtin meaning,
        // including the implication from `child == true` to the child itself.
        if !crate::checks::operators::closed_operators_are_builtin(
            self.program,
            &self.facts.operators,
            guard,
        ) {
            return false;
        }
        if self.equivalent(guard, required, Namespace::Callee, 0) {
            return true;
        }
        let Some(binary) = self.program.expression_table.binary_node(guard) else {
            return false;
        };
        match binary.operator {
            BinaryOperator::And => {
                self.conjunct(binary.left, required) || self.conjunct(binary.right, required)
            }
            BinaryOperator::Equal
                if matches!(
                    self.program.expression_table.expression(binary.right),
                    ExpressionNode::Boolean(true)
                ) =>
            {
                self.conjunct(binary.left, required)
            }
            _ => false,
        }
    }

    fn copied_source(&self, expression: ExpressionHandle) -> Option<ExpressionHandle> {
        if !matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
        ) {
            return None;
        }
        let place = validation::canonical_place_from_expression_in_state(
            self.program,
            self.state.state_symbol,
            self.call.statement_index,
            expression,
        )?;
        self.aliases
            .iter()
            .find(|(destination, _)| same_place(self.program, &place, destination))
            .map(|(_, source)| *source)
    }

    fn equivalent(
        &self,
        guard: ExpressionHandle,
        required: ExpressionHandle,
        namespace: Namespace,
        depth: usize,
    ) -> bool {
        if depth >= 64 {
            return false;
        }
        // Substitution is simultaneous and identity-based. An actual is already
        // in caller scope, even for a recursive edge sharing parameter symbols.
        // Printed names are diagnostics: a const binder and a later local can
        // legally share a spelling while denoting different observations.
        if namespace == Namespace::Callee
            && let ExpressionNode::Name(path) = self.program.expression_table.expression(required)
            && self
                .program
                .expression_table
                .name_path_members(path.members)
                .len()
                == 1
            && let Some(position) = self.parameters.iter().position(|parameter| {
                path.head_symbol == parameter.symbol || path.symbol == parameter.symbol
            })
        {
            let Some(mut argument) = self.arguments.get(position).copied() else {
                return false;
            };
            if matches!(
                self.program
                    .type_reference_table
                    .type_reference(self.parameters[position].type_reference),
                typed_trees::types::TypeReferenceNode::Reference { .. }
            ) && let ExpressionNode::Borrow(borrow) =
                self.program.expression_table.expression(argument)
            {
                argument = borrow.target;
            }
            return self.equivalent(guard, argument, Namespace::Caller, depth + 1);
        }
        if let (Some(left), Some(right)) = (
            self.closed_integer(guard, Namespace::Caller),
            self.closed_integer(required, namespace),
        ) {
            return left == right;
        }
        let recurse = |left, right| self.equivalent(left, right, namespace, depth + 1);
        let same_observation = match (
            self.program.expression_table.expression(guard),
            self.program.expression_table.expression(required),
        ) {
            (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
                left.resolved_symbol().is_some()
                    && left.resolved_symbol() == right.resolved_symbol()
                    && left.symbol == right.symbol
                    && self
                        .program
                        .expression_table
                        .name_path_members(left.members)
                        == self
                            .program
                            .expression_table
                            .name_path_members(right.members)
            }
            (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
                left.member_symbol.is_valid()
                    && left.member_symbol == right.member_symbol
                    && left.case_variant == right.case_variant
                    && recurse(left.receiver, right.receiver)
            }
            (ExpressionNode::Indexed(left), ExpressionNode::Indexed(right)) => {
                recurse(left.collection, right.collection) && recurse(left.index, right.index)
            }
            (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left
                .value_bignum()
                .is_some_and(|value| Some(value) == right.value_bignum()),
            (ExpressionNode::Float(left), ExpressionNode::Float(right)) => left == right,
            (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
            (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
                left.operator == right.operator
                    && crate::checks::operators::closed_operators_are_builtin(
                        self.program,
                        &self.facts.operators,
                        guard,
                    )
                    && crate::checks::operators::closed_operators_are_builtin(
                        self.program,
                        &self.facts.operators,
                        required,
                    )
                    && recurse(left.left, right.left)
                    && recurse(left.right, right.right)
            }
            (ExpressionNode::Unary(left), ExpressionNode::Unary(right)) => {
                left.operator == right.operator
                    && crate::checks::operators::closed_operators_are_builtin(
                        self.program,
                        &self.facts.operators,
                        guard,
                    )
                    && crate::checks::operators::closed_operators_are_builtin(
                        self.program,
                        &self.facts.operators,
                        required,
                    )
                    && recurse(left.operand, right.operand)
            }
            // A new structural form needs its own identity judgment. Never
            // replace an unsupported observation with an equal printed label.
            _ => false,
        };
        // Current observations exist independently of their producers. Keep
        // their direct identity before expanding retained copy equations;
        // match/call-produced locals need not be expressible by this algebra.
        same_observation
            || self
                .copied_source(guard)
                .is_some_and(|source| self.equivalent(source, required, namespace, depth + 1))
            || (namespace == Namespace::Caller
                && self
                    .copied_source(required)
                    .is_some_and(|source| self.equivalent(guard, source, namespace, depth + 1)))
    }

    /// Fold selected closed integer values, including fixed extents. No live
    /// inputs are supplied, so a stored value or dynamic count stays unknown.
    fn closed_integer(
        &self,
        expression: ExpressionHandle,
        namespace: Namespace,
    ) -> Option<numerics::bignum::BigInt> {
        if let ExpressionNode::Integer(literal) =
            self.program.expression_table.expression(expression)
        {
            return literal.value_bignum();
        }
        let (machine, state) = match namespace {
            Namespace::Caller => validation::machine_state_by_symbols(
                self.program,
                self.state.machine_symbol,
                self.state.state_symbol,
            )?,
            Namespace::Callee => validation::find_state(self.program, self.call.target_symbol)
                .or_else(|| validation::find_machine_head(self.program, self.call.target_symbol))?,
        };
        let primitive = if validation::collection_length_receiver(
            self.program,
            machine,
            Some(state),
            expression,
        )
        .is_some()
        {
            typed_trees::types::PrimitiveType::U64
        } else {
            let reference = validation::expression_result_type_reference(
                self.program,
                machine,
                state,
                expression,
            )?;
            self.program.primitive_type_reference(reference)?
        };
        if !primitive.is_fixed_integer() {
            return None;
        }
        let value = crate::values::lower_unit_scalar_argument(
            self.program,
            &self.facts.operators,
            state,
            if namespace == Namespace::Caller {
                self.call.statement_index
            } else {
                0
            },
            expression,
            primitive,
        )?;
        let facts::ScalarValue::Integer(value) =
            crate::values::evaluate_checked_scalar(&value, &mut |_| None)?
        else {
            return None;
        };
        Some(value)
    }
}

fn same_place(
    program: &dyn ::validation::CheckingInput,
    first: &validation::CanonicalPlace,
    second: &validation::CanonicalPlace,
) -> bool {
    first.root == second.root
        && first.segments.len() == second.segments.len()
        && first
            .segments
            .iter()
            .zip(&second.segments)
            .all(|(left, right)| {
                crate::flow::canonical_place_segments_equal(program, *left, *right)
            })
}

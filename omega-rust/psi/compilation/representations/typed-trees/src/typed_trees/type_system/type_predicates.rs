//! Static equality consumes exact normalized types, never stored values.

use crate::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
    types::TypeReferenceHandle,
};

impl TypedTrees {
    pub fn static_type_predicate_operands(
        &self,
        expression: ExpressionHandle,
    ) -> Option<(TypeReferenceHandle, TypeReferenceHandle, bool)> {
        let ExpressionNode::Binary(binary) = self.expression_table.expression(expression) else {
            return None;
        };
        if !matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::NotEqual
        ) {
            return None;
        }
        let ExpressionNode::TypeExpression(left) = self.expression_table.expression(binary.left)
        else {
            return None;
        };
        let ExpressionNode::TypeExpression(right) = self.expression_table.expression(binary.right)
        else {
            return None;
        };
        Some((*left, *right, binary.operator == BinaryOperator::NotEqual))
    }

    /// Close a declaration predicate against supplied arguments, before body
    /// substitution can replace a formal by its assumed where definition.
    /// No branch assumptions participate; every leaf must have a closed type.
    pub fn applied_static_type_predicate_value(
        &self,
        expression: ExpressionHandle,
        substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
    ) -> Option<bool> {
        if let Some((left, right, negate)) = self.static_type_predicate_operands(expression) {
            if !crate::domain::index_argument_is_closed_with_substitutions(
                self,
                left,
                substitutions,
            ) || !crate::domain::index_argument_is_closed_with_substitutions(
                self,
                right,
                substitutions,
            ) {
                return None;
            }
            let identity = |reference| {
                self.type_identity(crate::type_identity::TypeIdentityRequest {
                    substitutions,
                    ..crate::type_identity::TypeIdentityRequest::ordinary(reference)
                })
            };
            return Some((identity(left) == identity(right)) != negate);
        }
        match self.expression_table.expression(expression) {
            ExpressionNode::Binary(binary)
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
            {
                // Form the entire predicate even when a sibling is true.
                let left = self.applied_static_type_predicate_value(binary.left, substitutions)?;
                let right =
                    self.applied_static_type_predicate_value(binary.right, substitutions)?;
                Some(if binary.operator == BinaryOperator::And {
                    left && right
                } else {
                    left || right
                })
            }
            ExpressionNode::Unary(unary)
                if unary.operator == crate::expression::UnaryOperator::LogicalNot =>
            {
                self.applied_static_type_predicate_value(unary.operand, substitutions)
                    .map(|value| !value)
            }
            _ => None,
        }
    }

    pub fn static_type_predicate_value(&self, expression: ExpressionHandle) -> Option<bool> {
        self.static_type_predicate_value_in(expression, expression)
    }

    /// Observe a type proposition in a specific already-formed source path.
    /// Only that path's where/guard-derived environment refines the operands;
    /// the requested proposition supplies no premise of its own.
    pub fn static_type_predicate_value_in(
        &self,
        expression: ExpressionHandle,
        context: ExpressionHandle,
    ) -> Option<bool> {
        let (left, right, negate) = self.static_type_predicate_operands(expression)?;
        let left = self.statically_refined_type_reference(context, left);
        let right = self.statically_refined_type_reference(context, right);
        if !self.type_reference_table.contains_type_reference(left)
            || !self.type_reference_table.contains_type_reference(right)
            || !crate::domain::index_argument_is_closed(self, left)
            || !crate::domain::index_argument_is_closed(self, right)
        {
            return None;
        }
        Some(
            (self.normalized_type_identity(left) == self.normalized_type_identity(right)) != negate,
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct StaticTypeContexts {
    boolean_equalities: Vec<ExpressionHandle>,
    rows: Vec<(ExpressionHandle, Vec<TypeWorld>)>,
    transition_rows: Vec<(crate::statement::TransitionTargetHandle, Vec<TypeWorld>)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
struct TypeWorld {
    bindings: Vec<(symbols::SymbolHandle, TypeReferenceHandle)>,
    exclusions: Vec<(symbols::SymbolHandle, TypeReferenceHandle)>,
}

impl StaticTypeContexts {
    pub fn refined_reference(
        &self,
        program: &TypedTrees,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        let Ok(index) = self.rows.binary_search_by_key(
            &(expression.arena_index(), expression.generation()),
            |(owned, _)| (owned.arena_index(), owned.generation()),
        ) else {
            return reference;
        };
        Self::refined_in_worlds(program, reference, &self.rows[index].1)
    }

    fn refined_in_worlds(
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        worlds: &[TypeWorld],
    ) -> TypeReferenceHandle {
        let mut result = None;
        for world in worlds {
            let current = world.resolve(program, reference);
            if let Some(previous) = result {
                if program.normalized_type_identity(previous)
                    != program.normalized_type_identity(current)
                {
                    return reference;
                }
            } else {
                result = Some(current);
            }
        }
        result.unwrap_or(reference)
    }

    pub fn is_unreachable(&self, expression: ExpressionHandle) -> bool {
        self.rows
            .binary_search_by_key(
                &(expression.arena_index(), expression.generation()),
                |(owned, _)| (owned.arena_index(), owned.generation()),
            )
            .is_ok_and(|index| self.rows[index].1.is_empty())
    }
}

#[derive(Default)]
struct StaticContextBuilder {
    rows: std::collections::HashMap<ExpressionHandle, Vec<TypeWorld>, symbols::BuildSymbolHasher>,
    transition_rows: std::collections::HashMap<
        crate::statement::TransitionTargetHandle,
        Vec<TypeWorld>,
        symbols::BuildSymbolHasher,
    >,
}
impl StaticContextBuilder {
    fn record(&mut self, expression: ExpressionHandle, worlds: &[TypeWorld]) {
        let previous = self.rows.entry(expression).or_default();
        for world in worlds {
            if !previous.contains(world) {
                previous.push(world.clone());
            }
        }
    }
}

impl TypeWorld {
    fn resolve(
        &self,
        program: &TypedTrees,
        mut reference: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        let mut seen = Vec::new();
        while !seen.contains(&reference)
            && program
                .type_reference_table
                .contains_type_reference(reference)
        {
            seen.push(reference);
            let crate::types::TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
            else {
                break;
            };
            let Some((_, replacement)) = self
                .bindings
                .iter()
                .rev()
                .find(|(binder, _)| binder == symbol)
            else {
                break;
            };
            reference = *replacement;
        }
        reference
    }

    fn constrain(
        mut self,
        program: &TypedTrees,
        left: TypeReferenceHandle,
        right: TypeReferenceHandle,
        equal: bool,
    ) -> Option<Self> {
        let left = self.resolve(program, left);
        let right = self.resolve(program, right);
        if program.normalized_type_identity(left) == program.normalized_type_identity(right) {
            return equal.then_some(self);
        }
        let binder = |reference| match program.type_reference_table.type_reference(reference) {
            crate::types::TypeReferenceNode::Named { symbol, .. }
                if program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter =>
            {
                Some(*symbol)
            }
            _ => None,
        };
        let (symbol, value) = if let Some(symbol) = binder(left) {
            (symbol, right)
        } else if let Some(symbol) = binder(right) {
            (symbol, left)
        } else {
            if crate::domain::index_argument_is_closed(program, left)
                && crate::domain::index_argument_is_closed(program, right)
            {
                return (!equal).then_some(self);
            }
            // An open structural comparison remains undecided, never false merely
            // because its current binder identities differ.
            return Some(self);
        };
        if equal {
            if self.exclusions.iter().any(|(excluded, reference)| {
                *excluded == symbol
                    && program.normalized_type_identity(*reference)
                        == program.normalized_type_identity(value)
            }) {
                return None;
            }
            self.bindings.push((symbol, value));
        } else {
            self.exclusions.push((symbol, value));
        }
        Some(self)
    }
}

fn condition(
    program: &TypedTrees,
    worlds: &[TypeWorld],
    expression: ExpressionHandle,
    holds: bool,
) -> Vec<TypeWorld> {
    if !contains_type_predicate(program, expression, &mut Vec::new()) {
        return worlds.to_vec();
    }
    condition_inner(program, worlds, expression, holds)
}

fn contains_type_predicate(
    program: &TypedTrees,
    expression: ExpressionHandle,
    active: &mut Vec<ExpressionHandle>,
) -> bool {
    if !program.expression_table.expression_is_valid(expression) || active.contains(&expression) {
        return false;
    }
    if program.static_type_predicate_operands(expression).is_some() {
        return true;
    }
    active.push(expression);
    let result = match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary)
            if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                || program
                    .static_type_contexts
                    .boolean_equalities
                    .contains(&expression) =>
        {
            contains_type_predicate(program, binary.left, active)
                || contains_type_predicate(program, binary.right, active)
        }
        ExpressionNode::Unary(unary)
            if unary.operator == crate::expression::UnaryOperator::LogicalNot =>
        {
            contains_type_predicate(program, unary.operand, active)
        }
        ExpressionNode::Name(path)
            if path.symbol.is_valid()
                && program.symbols.get(path.symbol).kind == symbols::SymbolKind::Local =>
        {
            let state_symbol = program.symbols.get(path.symbol).parent;
            let machine_symbol = program.symbols.get(state_symbol).parent;
            program
                .machine_by_symbol(machine_symbol)
                .and_then(|machine| program.state_in_machine(machine, state_symbol))
                .and_then(|state| program.state_local_declaration(state, path.symbol))
                .filter(|(_, local)| !local.is_mutable)
                .and_then(|(_, local)| local.ordinary_value_initializer())
                .is_some_and(|initializer| contains_type_predicate(program, initializer, active))
        }
        _ => false,
    };
    active.pop();
    result
}

fn condition_inner(
    program: &TypedTrees,
    worlds: &[TypeWorld],
    expression: ExpressionHandle,
    holds: bool,
) -> Vec<TypeWorld> {
    if !program.expression_table.expression_is_valid(expression) {
        return worlds.to_vec();
    }
    if let Some((left, right, negate)) = program.static_type_predicate_operands(expression) {
        return worlds
            .iter()
            .cloned()
            .filter_map(|world| world.constrain(program, left, right, holds != negate))
            .collect();
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path)
            if path.symbol.is_valid()
                && program.symbols.get(path.symbol).kind == symbols::SymbolKind::Local =>
        {
            let state_symbol = program.symbols.get(path.symbol).parent;
            let machine_symbol = program.symbols.get(state_symbol).parent;
            if let Some(machine) = program.machine_by_symbol(machine_symbol)
                && let Some(state) = program.state_in_machine(machine, state_symbol)
                && let Some((_, local)) = program.state_local_declaration(state, path.symbol)
                && !local.is_mutable
                && let Some(initializer) = local.ordinary_value_initializer()
            {
                condition_inner(program, worlds, initializer, holds)
            } else {
                worlds.to_vec()
            }
        }
        ExpressionNode::Binary(binary)
            if binary.operator == BinaryOperator::CaseMembership
                || program
                    .static_type_contexts
                    .boolean_equalities
                    .binary_search_by_key(
                        &(expression.arena_index(), expression.generation()),
                        |handle| (handle.arena_index(), handle.generation()),
                    )
                    .is_ok() =>
        {
            let wrapped = match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (_, ExpressionNode::Boolean(value)) => Some((binary.left, *value)),
                (ExpressionNode::Boolean(value), _) => Some((binary.right, *value)),
                _ => None,
            };
            if let Some((subject, value)) = wrapped {
                condition_inner(
                    program,
                    worlds,
                    subject,
                    holds == (value != (binary.operator == BinaryOperator::NotEqual)),
                )
            } else {
                worlds.to_vec()
            }
        }
        ExpressionNode::Boolean(value) => {
            if *value == holds {
                worlds.to_vec()
            } else {
                Vec::new()
            }
        }
        ExpressionNode::Unary(unary)
            if unary.operator == crate::expression::UnaryOperator::LogicalNot =>
        {
            condition_inner(program, worlds, unary.operand, !holds)
        }
        ExpressionNode::Binary(binary)
            if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
        {
            let conjunction = binary.operator == BinaryOperator::And;
            if holds == conjunction {
                condition_inner(
                    program,
                    &condition_inner(program, worlds, binary.left, holds),
                    binary.right,
                    holds,
                )
            } else {
                let mut alternatives = condition_inner(program, worlds, binary.left, holds);
                alternatives.extend(condition_inner(
                    program,
                    &condition_inner(program, worlds, binary.left, !holds),
                    binary.right,
                    holds,
                ));
                alternatives
            }
        }
        _ => worlds.to_vec(),
    }
}

fn visit_context(
    program: &TypedTrees,
    output: &mut StaticContextBuilder,
    expression: ExpressionHandle,
    worlds: &[TypeWorld],
    active: &mut Vec<ExpressionHandle>,
) {
    if !program.expression_table.expression_is_valid(expression) || active.contains(&expression) {
        return;
    }
    output.record(expression, worlds);
    active.push(expression);
    let mut visit = |child, environment: &[TypeWorld]| {
        visit_context(program, output, child, environment, active)
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            visit(binary.left, worlds);
            let right = if binary.operator == BinaryOperator::And {
                condition(program, worlds, binary.left, true)
            } else if binary.operator == BinaryOperator::Or {
                condition(program, worlds, binary.left, false)
            } else {
                worlds.to_vec()
            };
            visit(binary.right, &right);
        }
        ExpressionNode::Match(dispatch) => {
            visit(dispatch.subject, worlds);
            let mut remaining = worlds.to_vec();
            for arm in program.expression_table.match_arms(dispatch.arms) {
                let environment = match arm.pattern {
                    crate::expression::MatchPattern::Value(pattern) => {
                        visit(pattern, &remaining);
                        if let ExpressionNode::Boolean(value) =
                            program.expression_table.expression(pattern)
                        {
                            let environment =
                                condition(program, &remaining, dispatch.subject, *value);
                            remaining = condition(program, &remaining, dispatch.subject, !*value);
                            environment
                        } else {
                            remaining.clone()
                        }
                    }
                    _ => remaining.clone(),
                };
                visit(arm.value, &environment);
            }
        }
        ExpressionNode::Call(call) => {
            visit(call.receiver, worlds);
            for &argument in program.expression_table.expression_handles(call.arguments) {
                visit(argument, worlds);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            visit(atomic.value, worlds);
            visit(atomic.result, worlds);
        }
        ExpressionNode::Unary(unary) => visit(unary.operand, worlds),
        ExpressionNode::Borrow(borrow) => visit(borrow.target, worlds),
        ExpressionNode::Cast(cast) => visit(cast.value, worlds),
        ExpressionNode::Member(member) => visit(member.receiver, worlds),
        ExpressionNode::Indexed(indexed) => {
            visit(indexed.collection, worlds);
            visit(indexed.index, worlds);
        }
        ExpressionNode::Range(range) => {
            visit(range.start, worlds);
            visit(range.end, worlds);
        }
        ExpressionNode::ArrayLiteral(values) => {
            for &value in program.expression_table.expression_handles(*values) {
                visit(value, worlds);
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                visit(field.value, worlds);
            }
        }
        ExpressionNode::SavedSubject(_)
        | ExpressionNode::TypeExpression(_)
        | crate::literal_expression_pattern!() => {}
    }
    active.pop();
}

impl TypedTrees {
    /// Exact type assumptions belong to the declaration and the branch that
    /// establishes them. Rebuild after specialization; no caller chooses the
    /// assumptions of an unspecialized template.
    pub fn prepare_static_type_contexts(&mut self, boolean_equalities: &[ExpressionHandle]) {
        self.static_type_contexts.boolean_equalities = boolean_equalities.to_vec();
        use crate::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
        let mut contexts = StaticContextBuilder::default();
        for machine in self.machines() {
            let mut worlds = vec![TypeWorld::default()];
            for equation in self
                .declared_type_equations
                .span_or_empty(machine.declared_type_equations)
            {
                worlds = worlds
                    .into_iter()
                    .filter_map(|world| {
                        world.constrain(self, equation.parameter, equation.definition, true)
                    })
                    .collect();
            }
            for &predicate in self
                .expression_table
                .expression_handles(machine.declared_type_predicates)
            {
                worlds = condition(self, &worlds, predicate, true);
            }
            let mut active = Vec::new();
            for contract in self.machine_contracts(machine) {
                for fact in self.proof_facts.span_or_empty(contract.facts) {
                    if let crate::domain::ProofFact::Expression(expression) = fact {
                        // Form each requirement under prior premises before its
                        // static equations become available to later contracts
                        // and the body. A goal cannot supply its own type facts.
                        visit_context(self, &mut contexts, *expression, &worlds, &mut active);
                        if contract.kind == crate::signature::SignatureContractKind::Requires {
                            worlds = condition(self, &worlds, *expression, true);
                        }
                    }
                }
            }
            for state in self.machine_states(machine) {
                let mut remaining = worlds.clone();
                for contract in self.state_contracts(state) {
                    for fact in self.proof_facts.span_or_empty(contract.facts) {
                        if let crate::domain::ProofFact::Expression(expression) = fact {
                            visit_context(
                                self,
                                &mut contexts,
                                *expression,
                                &remaining,
                                &mut active,
                            );
                            if contract.kind == crate::signature::SignatureContractKind::Requires {
                                remaining = condition(self, &remaining, *expression, true);
                            }
                        }
                    }
                }
                for statement in self.statement_table.statements(state.statement_nodes) {
                    let mut visit = |expression, environment: &[TypeWorld]| {
                        visit_context(self, &mut contexts, expression, environment, &mut active)
                    };
                    match statement {
                        StatementNode::Transition(transition) => {
                            let selected = match transition.guard {
                                TransitionGuardNode::Always => remaining.clone(),
                                TransitionGuardNode::When(guard) => {
                                    visit(guard, &remaining);
                                    condition(self, &remaining, guard, true)
                                }
                            };
                            let complement = match transition.guard {
                                TransitionGuardNode::Always => Vec::new(),
                                TransitionGuardNode::When(guard) => {
                                    condition(self, &remaining, guard, false)
                                }
                            };
                            for target in transition.targets() {
                                let target_worlds = if target == transition.target {
                                    &selected
                                } else {
                                    &complement
                                };
                                let retained = contexts.transition_rows.entry(target).or_default();
                                for world in target_worlds {
                                    if !retained.contains(world) {
                                        retained.push(world.clone());
                                    }
                                }
                                match self.statement_table.transition_target(target) {
                                    TransitionTargetNode::Value(value) => visit_context(
                                        self,
                                        &mut contexts,
                                        *value,
                                        target_worlds,
                                        &mut active,
                                    ),
                                    TransitionTargetNode::Named { arguments, .. } => {
                                        for &argument in
                                            self.statement_table.expression_handles(*arguments)
                                        {
                                            visit_context(
                                                self,
                                                &mut contexts,
                                                argument,
                                                target_worlds,
                                                &mut active,
                                            );
                                        }
                                    }
                                    TransitionTargetNode::Terminal => {}
                                }
                            }
                            remaining = match transition.guard {
                                TransitionGuardNode::Always => remaining.clone(),
                                TransitionGuardNode::When(guard) => {
                                    condition(self, &remaining, guard, false)
                                }
                            };
                        }
                        StatementNode::Expression(expression) => visit(*expression, &remaining),
                        StatementNode::LocalData(local) => {
                            if let Some(value) = local.ordinary_value_initializer() {
                                visit(value, &remaining);
                            }
                        }
                        StatementNode::Assignment(assignment) => {
                            visit(assignment.target, &remaining);
                            visit(assignment.value, &remaining);
                        }
                        StatementNode::Call(call) => {
                            for &argument in self.statement_table.expression_handles(call.arguments)
                            {
                                visit(argument, &remaining);
                            }
                        }
                        StatementNode::AssemblyFact(fact) => {
                            if let crate::domain::ProofFact::Expression(expression) =
                                self.proof_facts.get(fact.fact)
                            {
                                visit(*expression, &remaining);
                            }
                        }
                        StatementNode::Assembly(_) => {}
                    }
                }
            }
        }
        let mut rows = contexts.rows.into_iter().collect::<Vec<_>>();
        rows.sort_unstable_by_key(|(expression, _)| {
            (expression.arena_index(), expression.generation())
        });
        self.static_type_contexts.rows = rows;
        let mut transition_rows = contexts.transition_rows.into_iter().collect::<Vec<_>>();
        transition_rows
            .sort_unstable_by_key(|(target, _)| (target.arena_index(), target.generation()));
        self.static_type_contexts.transition_rows = transition_rows;
    }

    pub fn static_transition_type_context_is_unreachable(
        &self,
        target: crate::statement::TransitionTargetHandle,
    ) -> bool {
        self.static_type_contexts
            .transition_rows
            .binary_search_by_key(
                &(target.arena_index(), target.generation()),
                |(owned, _)| (owned.arena_index(), owned.generation()),
            )
            .is_ok_and(|index| {
                self.static_type_contexts.transition_rows[index]
                    .1
                    .is_empty()
            })
    }

    pub fn statically_refined_transition_type_reference(
        &self,
        target: crate::statement::TransitionTargetHandle,
        reference: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        let Ok(index) = self
            .static_type_contexts
            .transition_rows
            .binary_search_by_key(
                &(target.arena_index(), target.generation()),
                |(owned, _)| (owned.arena_index(), owned.generation()),
            )
        else {
            return reference;
        };
        StaticTypeContexts::refined_in_worlds(
            self,
            reference,
            &self.static_type_contexts.transition_rows[index].1,
        )
    }

    pub fn statically_refined_type_reference(
        &self,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        self.static_type_contexts
            .refined_reference(self, expression, reference)
    }
    pub fn static_type_context_is_unreachable(&self, expression: ExpressionHandle) -> bool {
        self.static_type_contexts.is_unreachable(expression)
    }
}

impl TypedTrees {
    /// A proof consumer may project a statically selected Boolean branch.
    /// The canonical expression remains intact for independent formation.
    pub fn static_boolean_value(&self, expression: ExpressionHandle) -> Option<bool> {
        self.static_boolean_value_inner(expression, &mut Vec::new())
    }

    fn static_boolean_value_inner(
        &self,
        expression: ExpressionHandle,
        active: &mut Vec<ExpressionHandle>,
    ) -> Option<bool> {
        if !self.expression_table.expression_is_valid(expression) || active.contains(&expression) {
            return None;
        }
        if let Some(value) = self.static_type_predicate_value(expression) {
            return Some(value);
        }
        active.push(expression);
        let value = (|| match self.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(*value),
            ExpressionNode::Name(path)
                if path.symbol.is_valid()
                    && self.symbols.get(path.symbol).kind == symbols::SymbolKind::Local =>
            {
                let state_symbol = self.symbols.get(path.symbol).parent;
                let machine_symbol = self.symbols.get(state_symbol).parent;
                let machine = self.machine_by_symbol(machine_symbol)?;
                let state = self.state_in_machine(machine, state_symbol)?;
                let (_, local) = self.state_local_declaration(state, path.symbol)?;
                if local.is_mutable {
                    return None;
                }
                self.static_boolean_value_inner(local.ordinary_value_initializer()?, active)
            }
            ExpressionNode::Unary(unary)
                if unary.operator == crate::expression::UnaryOperator::LogicalNot =>
            {
                self.static_boolean_value_inner(unary.operand, active)
                    .map(|value| !value)
            }
            ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
                if self.static_boolean_value_inner(binary.left, active)? {
                    self.static_boolean_value_inner(binary.right, active)
                } else {
                    Some(false)
                }
            }
            ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Or => {
                if self.static_boolean_value_inner(binary.left, active)? {
                    Some(true)
                } else {
                    self.static_boolean_value_inner(binary.right, active)
                }
            }
            ExpressionNode::Binary(binary)
                if self
                    .static_type_contexts
                    .boolean_equalities
                    .contains(&expression) =>
            {
                let left = self.static_boolean_value_inner(binary.left, active)?;
                let right = self.static_boolean_value_inner(binary.right, active)?;
                Some((left == right) != (binary.operator == BinaryOperator::NotEqual))
            }
            _ => None,
        })();
        active.pop();
        value
    }

    /// Return an existing equivalent clause, never a manufactured scalar or
    /// a new source execution root. Formation still visits the whole original.
    pub fn static_boolean_normal_form(&self, mut expression: ExpressionHandle) -> ExpressionHandle {
        loop {
            let ExpressionNode::Binary(binary) = self.expression_table.expression(expression)
            else {
                return expression;
            };
            let Some(left) = self.static_boolean_value(binary.left) else {
                return expression;
            };
            expression = match binary.operator {
                BinaryOperator::And => {
                    if left {
                        binary.right
                    } else {
                        binary.left
                    }
                }
                BinaryOperator::Or => {
                    if left {
                        binary.left
                    } else {
                        binary.right
                    }
                }
                _ => return expression,
            };
        }
    }
}

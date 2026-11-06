//! Compare guard observations in the caller's current namespace. A captured
//! local expands only through its exact selected producer and a complete frame
//! preserving both the destination and every observed backing place. A reference
//! actual — a local loan or a returned loan — denotes that loan's storage, so
//! a field fact on the storage proves the formal's field requirement. Several
//! exact storages do not: the callee may receive any of them. Expansion
//! establishes denotation; it never evaluates a user call or assumes its purity.

use crate::checks::contracts::calls::computed_values_have_source_identity;
use crate::flow::state_statements;
use crate::published::facts::contract_occurrences::append_expression_occurrences;
use crate::published::places::place_at;
use checked_trees::FlowCallFact;
use checked_trees::{
    CheckFacts, CheckedScalarComputationKind as Computation, CheckedScalarExpressionRole,
};
use symbols::SymbolHandle;
use typed_trees::{
    expression::{ExpressionHandle, ExpressionNode},
    statement::StatementNode,
    types::PrimitiveType,
};

enum LoanStorage {
    One(validation::CanonicalPlace),
    Many,
    Unknown,
}

pub(in crate::checks::contracts) struct Denotations<'a> {
    pub program: &'a dyn ::validation::CheckingInput,
    pub facts: &'a CheckFacts,
    pub caller_state_symbol: SymbolHandle,
    pub target_state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub call_frames: Option<&'a validation::CallFrameResolver<'a>>,
    pub arguments: &'a [ExpressionHandle],
    pub call: &'a FlowCallFact,
    // Immutable query-local scratch, prepared only when a stored scalar alias
    // is actually needed. Exact observations and Boolean captures need no scan.
    pub copies: std::cell::OnceCell<Vec<(validation::CanonicalPlace, ExpressionHandle)>>,
}

impl Denotations<'_> {
    pub fn equal(&self, left: ExpressionHandle, right: ExpressionHandle) -> bool {
        self.compare(left, right, true, &mut 4096, 0)
    }

    fn compare(
        &self,
        left: ExpressionHandle,
        right: ExpressionHandle,
        formal: bool,
        remaining: &mut usize,
        depth: usize,
    ) -> bool {
        if depth >= 64 || *remaining == 0 {
            return false;
        }
        *remaining -= 1;
        let table = &self.program.expression_table;
        // Enter an actual exactly once, even for recursion where caller and
        // callee formal symbols share a declaration namespace.
        if formal
            && let ExpressionNode::Name(path) = table.expression(right)
            && table.name_path_is_single_member(path)
            && let Some(position) = validation::call_contract_parameter_position(
                self.program,
                self.target_state_symbol,
                path.symbol,
            )
        {
            return self
                .arguments
                .get(position)
                .is_some_and(|actual| self.compare(left, *actual, false, remaining, depth + 1));
        }
        if let Some(source) = self.captured(left, remaining) {
            return self.compare(source, right, formal, remaining, depth + 1);
        }
        if !formal && let Some(source) = self.captured(right, remaining) {
            return self.compare(left, source, false, remaining, depth + 1);
        }
        // A borrow actual supplies its referent's observation to the formal.
        // Remove only borrowing syntax: canonical place equality would also
        // erase recasts, whose scalar interpretation must remain distinct.
        if !formal {
            if let ExpressionNode::Borrow(borrow) = table.expression(left) {
                return self.compare(borrow.target, right, false, remaining, depth + 1);
            }
            if let ExpressionNode::Borrow(borrow) = table.expression(right) {
                return self.compare(left, borrow.target, false, remaining, depth + 1);
            }
        }
        if formal
            && let Some(required) = crate::flow::projected_leaf_place(self.program, right)
            && let Some(root) = required.root.as_symbol()
            && let Some(position) = validation::call_contract_parameter_position(
                self.program,
                self.target_state_symbol,
                root,
            )
            && let Some(actual) = self.arguments.get(position)
            && let Some(observed) = crate::flow::projected_leaf_place(self.program, left)
        {
            // A borrow spelled at the call already is the storage place.
            // A local or returned loan is a different expression for that
            // same storage; compare the loan's exact referent, not its name.
            if let Some(mut syntactic) = crate::flow::projected_leaf_place(self.program, *actual) {
                syntactic.extend_segments(&required.segments);
                if syntactic == observed {
                    return true;
                }
            }
            if self.expression_is_reference_loan(*actual) {
                match self.exact_loan_storage(*actual) {
                    LoanStorage::One(mut storage) => {
                        storage.extend_segments(&required.segments);
                        return storage == observed;
                    }
                    LoanStorage::Many => return false,
                    LoanStorage::Unknown => {}
                }
            }
        }
        if let (Some(left), Some(right)) = (
            self.closed_integer(left, false),
            self.closed_integer(right, formal),
        ) {
            return left == right;
        }
        let equal = |left, right, remaining: &mut usize| {
            self.compare(left, right, formal, remaining, depth + 1)
        };
        let same_observation = match (table.expression(left), table.expression(right)) {
            (ExpressionNode::Unary(a), ExpressionNode::Unary(b)) => {
                a.operator == b.operator
                    && self.builtin(left)
                    && self.builtin(right)
                    && equal(a.operand, b.operand, remaining)
            }
            (ExpressionNode::Binary(a), ExpressionNode::Binary(b)) => {
                a.operator == b.operator
                    && self.same_binary_meaning(left, right, formal)
                    && equal(a.left, b.left, remaining)
                    && equal(a.right, b.right, remaining)
            }
            (ExpressionNode::Member(a), ExpressionNode::Member(b)) => {
                // The table owns resolved member identity; the enclosing
                // receivers may belong to different formal namespaces.
                a.member == b.member
                    && a.case_variant == b.case_variant
                    && a.member_symbol.is_valid()
                    && a.member_symbol == b.member_symbol
                    && equal(a.receiver, b.receiver, remaining)
            }
            (ExpressionNode::Indexed(a), ExpressionNode::Indexed(b)) => {
                equal(a.collection, b.collection, remaining) && equal(a.index, b.index, remaining)
            }
            (ExpressionNode::Borrow(a), ExpressionNode::Borrow(b)) => {
                a.access == b.access && equal(a.target, b.target, remaining)
            }
            (ExpressionNode::Call(a), ExpressionNode::Call(b)) => {
                // Specification predicates are applications of their exact
                // checked declaration, not fresh observations of ambient state.
                // Source formation separately owes totality. Shared reference
                // inputs retain their live contexts and capture epoch fences.
                if a.is_ordinary_positional_call()
                    && b.is_ordinary_positional_call()
                    && a.target_symbol.is_valid()
                    && a.target_symbol == b.target_symbol
                {
                    let target =
                        validation::find_state(self.program, a.target_symbol).or_else(|| {
                            let machine = self.program.machine_by_symbol(a.target_symbol)?;
                            Some((machine, self.program.machine_states(machine).first()?))
                        });
                    let call_plans = validation::operand_call_plans(self.program, self.call_frames);
                    let (operational, service_reaches) = call_plans.as_ref();
                    if target.is_some_and(|(machine, state)| {
                        validation::checked_value_call_candidate(
                            self.program,
                            machine,
                            state,
                            operational,
                            service_reaches,
                        )
                        .is_ok()
                    }) {
                        let first = table.expression_handles(a.arguments);
                        let second = table.expression_handles(b.arguments);
                        return first.len() == second.len()
                            && first
                                .iter()
                                .zip(second)
                                .all(|(first, second)| equal(*first, *second, remaining));
                    }
                }
                // Byte predicates have their separately sealed intrinsic
                // selection rather than a checked machine body.
                match (
                    typed_trees::byte_predicates::byte_predicate_call(self.program, left),
                    typed_trees::byte_predicates::byte_predicate_call(self.program, right),
                ) {
                    (Some((first, left)), Some((second, right))) => {
                        first == second && equal(left, right, remaining)
                    }
                    _ => false,
                }
            }
            (ExpressionNode::Name(_), ExpressionNode::Name(_))
            | (ExpressionNode::Boolean(_), ExpressionNode::Boolean(_))
            | (ExpressionNode::Integer(_), ExpressionNode::Integer(_))
            | (ExpressionNode::Float(_), ExpressionNode::Float(_))
            | (ExpressionNode::String(_), ExpressionNode::String(_)) => {
                table.expressions_structurally_equal(left, right)
            }
            _ => false,
        };
        same_observation
            || self
                .copy_source(left)
                .is_some_and(|source| self.compare(source, right, formal, remaining, depth + 1))
            || (!formal
                && self
                    .copy_source(right)
                    .is_some_and(|source| self.compare(left, source, false, remaining, depth + 1)))
    }

    fn copy_source(&self, expression: ExpressionHandle) -> Option<ExpressionHandle> {
        if !matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
        ) {
            return None;
        }
        let place = place_at(
            self.program,
            self.caller_state_symbol,
            self.statement_index,
            expression,
        )?;
        self.copies
            .get_or_init(|| {
                stable_copies(
                    self.program,
                    self.facts,
                    self.caller_state_symbol,
                    self.call,
                )
            })
            .iter()
            .find(|(destination, _)| same_place(self.program, &place, destination))
            .map(|(_, source)| *source)
    }

    // Closed selected integer values include fixed extents. No live input is
    // supplied, so a stored scalar or dynamic descriptor remains unknown.
    fn closed_integer(
        &self,
        expression: ExpressionHandle,
        formal: bool,
    ) -> Option<numerics::bignum::BigInt> {
        if let ExpressionNode::Integer(literal) =
            self.program.expression_table.expression(expression)
        {
            return literal.value_bignum();
        }
        let (machine, state) =
            match formal {
                false => validation::find_state(self.program, self.caller_state_symbol)?,
                true => validation::find_state(self.program, self.target_state_symbol).or_else(
                    || validation::find_machine_head(self.program, self.target_state_symbol),
                )?,
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
            if !formal { self.statement_index } else { 0 },
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

    fn expression_is_reference_loan(&self, expression: ExpressionHandle) -> bool {
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Call(_) => true,
            ExpressionNode::Name(path) => {
                self.name_is_reference_local(path.symbol, path.head_symbol)
            }
            _ => false,
        }
    }

    fn name_is_reference_local(&self, symbol: SymbolHandle, head_symbol: SymbolHandle) -> bool {
        if !symbol.is_valid() || symbol != head_symbol {
            return false;
        }
        let Some((_, state)) = validation::find_state(self.program, self.caller_state_symbol)
        else {
            return false;
        };
        let Some((_, local)) = self.program.state_local_declaration(state, symbol) else {
            return false;
        };
        crate::checks::type_reference_is_reference(self.program, local.type_reference)
    }

    /// One exact referent of a reference actual at this call. `Many` means the
    /// loan may name more than one storage, so one field fact cannot prove it.
    fn exact_loan_storage(&self, expression: ExpressionHandle) -> LoanStorage {
        let Some(frames) = self.call_frames else {
            return LoanStorage::Unknown;
        };
        let Some((machine, state)) = validation::find_state(self.program, self.caller_state_symbol)
        else {
            return LoanStorage::Unknown;
        };
        let Some(statement) = self
            .program
            .statement_table
            .statement_at(state, self.statement_index)
        else {
            return LoanStorage::Unknown;
        };
        let Some(origins) =
            frames.reference_expression_origins_before_statement(machine, statement, expression)
        else {
            return LoanStorage::Unknown;
        };
        let mut storages = Vec::new();
        for (root, segments) in origins {
            let place = validation::CanonicalPlace {
                root: facts::PlaceRoot::Symbol(root),
                segments,
            };
            crate::shared::collections::push_unique(&mut storages, place);
        }
        match storages.len() {
            0 => LoanStorage::Unknown,
            1 => LoanStorage::One(storages.remove(0)),
            _ => LoanStorage::Many,
        }
    }

    fn same_binary_meaning(
        &self,
        left: ExpressionHandle,
        right: ExpressionHandle,
        formal: bool,
    ) -> bool {
        if self.builtin(left) && self.builtin(right) {
            return true;
        }
        let selected = |expression, state| {
            let (machine, state) = validation::find_state(self.program, state).or_else(|| {
                let machine = self.program.machine_by_symbol(state)?;
                Some((machine, self.program.machine_states(machine).first()?))
            })?;
            let binary = self.program.expression_table.binary_node(expression)?;
            validation::sealed_float_comparison_declaration(
                self.program,
                machine,
                Some(state),
                expression,
                binary,
            )
        };
        // Both occurrences keep their authored scope, format and sealed
        // selection. Formal substitution changes operands, never the operator.
        selected(left, self.caller_state_symbol)
            .zip(selected(
                right,
                if formal {
                    self.target_state_symbol
                } else {
                    self.caller_state_symbol
                },
            ))
            .is_some_and(|(left, right)| left == right)
    }

    fn builtin(&self, expression: ExpressionHandle) -> bool {
        crate::shared::authored_selections::typed_operator_has_no_authored_selection(
            self.program,
            expression,
        )
    }

    fn captured(
        &self,
        expression: ExpressionHandle,
        remaining: &mut usize,
    ) -> Option<ExpressionHandle> {
        let path = self.program.expression_table.name_path_node(expression)?;
        if !self
            .program
            .expression_table
            .name_path_is_single_member(path)
        {
            return None;
        }
        let (machine, state) = validation::find_state(self.program, self.caller_state_symbol)?;
        let statements = state_statements(self.program, state.statement_nodes);
        let (definition, local) = self.program.state_local_declaration(state, path.symbol)?;
        if definition >= self.statement_index
            || local.is_mutable
            || self.program.primitive_type_reference(local.type_reference)
                != Some(PrimitiveType::Bool)
            || !local.initial_value.is_valid()
        {
            return None;
        }
        let binding_ordinal = statements[..definition]
            .iter()
            .filter(|node| {
                matches!(node,
            StatementNode::LocalData(local) if !local.is_mutable && local.initial_value.is_valid()
                && self.program.primitive_type_reference(local.type_reference).is_some())
            })
            .count();
        let role = CheckedScalarExpressionRole::LocalInitializer {
            binding_ordinal: u32::try_from(binding_ordinal).ok()?,
        };
        let plans = &self.facts.values.scalar_expressions;
        let mut bindings = plans.source_bindings.iter().filter(|(_, binding)| {
            binding.state == state.symbol
                && binding.statement_ordinal == definition as u32
                && binding.role == role
        });
        if let Some((_, binding)) = bindings.next() {
            if bindings.next().is_some()
                || binding.expression != local.initial_value
                || binding.destination != local.symbol
            {
                return None;
            }
            let mut selected = plans.expressions.iter().filter(|plan| {
                plan.state == state.symbol
                    && plan.statement_ordinal == definition as u32
                    && plan.role == role
            });
            selected.next()?;
            if selected.next().is_some() {
                return None;
            }
        } else {
            let plans = &self.facts.values.scalar_computations;
            let root = plans.root_at(state.symbol, u32::try_from(definition).ok()?, role)?;
            if root.machine != machine.symbol
                || !plans.nodes.is_valid(root.root)
                || plans.nodes.get(root.root).authored_root != local.initial_value
                || !self.selected_pure(root.root, remaining, 0)
            {
                return None;
            }
        }
        // Selected computation establishes what was captured; only the shared
        // saved-guard owner establishes whether it still denotes current
        // storage. Frame paths name backing origins, not local loan names.
        // Keep argument evaluation in that same epoch judgment: an arm operand
        // can overwrite the observed bytes before its consumer begins.
        let frames = self.call_frames?;
        let mut later_writes = Vec::new();
        for argument in self.arguments {
            later_writes.extend(
                frames
                    .expression_write_frame(machine, *argument)
                    .into_complete_paths()?,
            );
        }
        if !crate::flow::saved_guard_observations(
            self.program,
            machine,
            state,
            self.statement_index,
            expression,
            true,
            &later_writes,
            Some(frames),
        )
        .iter()
        .any(|observation| {
            observation.current_expression == local.initial_value && observation.value
        }) {
            return None;
        }
        Some(local.initial_value)
    }

    fn selected_pure(
        &self,
        handle: checked_trees::CheckedScalarComputationHandle,
        remaining: &mut usize,
        depth: usize,
    ) -> bool {
        let plans = &self.facts.values.scalar_computations;
        if depth >= 64 || *remaining == 0 || !plans.nodes.is_valid(handle) {
            return false;
        }
        *remaining -= 1;
        match &plans.nodes.get(handle).kind {
            Computation::Value(_) => plans.nodes.get(handle).value_source.is_valid(),
            Computation::ByteSequencePredicate {
                predicate,
                source_expression,
                ..
            } => {
                typed_trees::byte_predicates::byte_predicate_call(self.program, *source_expression)
                    .is_some_and(|(selected, _)| selected == *predicate)
            }
            Computation::StructuralField {
                source_expression,
                receiver_value,
                ..
            } => {
                // Existing storage is a read, not a replayed aggregate call.
                // The outer dependency walk retains its complete write fence.
                !receiver_value.is_valid()
                    && *source_expression == plans.nodes.get(handle).authored_root
                    && crate::flow::projected_leaf_place(self.program, *source_expression).is_some()
            }
            Computation::SelectedComparison {
                operator_use,
                left,
                right,
            } => {
                let Some(_) = self
                    .facts
                    .operators
                    .selected_float_comparison(self.program, *operator_use)
                else {
                    return false;
                };
                let selected = self.facts.operators.uses.get(*operator_use);
                let Some((machine, state)) =
                    validation::find_state(self.program, self.caller_state_symbol)
                else {
                    return false;
                };
                let Some(binary) = self
                    .program
                    .expression_table
                    .binary_node(selected.expression)
                else {
                    return false;
                };
                selected.expression == plans.nodes.get(handle).authored_root
                    && validation::sealed_float_comparison_declaration(
                        self.program,
                        machine,
                        Some(state),
                        selected.expression,
                        binary,
                    ) == Some(selected.selected_operator_symbol)
                    && self.selected_pure(*left, remaining, depth + 1)
                    && self.selected_pure(*right, remaining, depth + 1)
            }
            Computation::Apply { operands, .. } => {
                plans.operands.span(*operands).is_some_and(|operands| {
                    operands
                        .iter()
                        .all(|operand| self.selected_pure(*operand, remaining, depth + 1))
                })
            }
            _ => false,
        }
    }
}

fn stable_copies(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_symbol: SymbolHandle,
    call: &FlowCallFact,
) -> Vec<(validation::CanonicalPlace, ExpressionHandle)> {
    let Some((machine, state)) = validation::find_state(program, state_symbol) else {
        return Vec::new();
    };
    let Some(state) = facts.flow.control.state(machine.symbol, state.symbol) else {
        return Vec::new();
    };
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
            || !computed_values_have_source_identity(program, facts, stored.expression)
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
        append_expression_occurrences(program, stored.expression, &mut reads);
        let mut selectors_preserve_identity = true;
        for segment in &destination.segments {
            if let facts::PlaceSegment::Index { expression } = segment {
                selectors_preserve_identity &=
                    computed_values_have_source_identity(program, facts, *expression);
                append_expression_occurrences(program, *expression, &mut reads);
            }
        }
        if selectors_preserve_identity
            && reads.iter().all(|read| {
                place_at(program, state.state_symbol, stored.statement_index, *read).is_some_and(
                    |place| {
                        crate::flow::operand_observations_agree(
                            program,
                            &facts.flow,
                            stored,
                            invocation,
                            &place,
                        )
                    },
                )
            })
        {
            aliases.push((destination, stored.expression));
        }
    }
    aliases
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

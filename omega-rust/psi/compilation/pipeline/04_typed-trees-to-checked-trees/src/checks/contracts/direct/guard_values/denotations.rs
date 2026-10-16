//! Compare guard observations in the caller's current namespace. A captured
//! local expands only through its exact selected producer and a complete frame
//! preserving both the destination and every observed storage root. Expansion
//! establishes denotation; it never evaluates a user call or assumes its purity.

use checked_trees::{
    CheckFacts, CheckedScalarComputationKind as Computation, CheckedScalarExpressionRole,
};
use symbols::SymbolHandle;
use typed_trees::{
    expression::{ExpressionHandle, ExpressionNode},
    statement::StatementNode,
    types::PrimitiveType,
};

pub(in super::super) struct Denotations<'a> {
    pub program: &'a dyn ::validation::CheckingInput,
    pub facts: &'a CheckFacts,
    pub caller_state_symbol: SymbolHandle,
    pub target_state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub call_frames: Option<&'a validation::CallFrameResolver<'a>>,
    pub arguments: &'a [ExpressionHandle],
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
            && path.symbol == path.head_symbol
            && table.name_path_members(path.members).len() == 1
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
            && let Some(mut required) = crate::flow::projected_leaf_place(self.program, right)
            && let Some(root) = required.root.as_symbol()
            && let Some(position) = validation::call_contract_parameter_position(
                self.program,
                self.target_state_symbol,
                root,
            )
            && let Some(actual) = self.arguments.get(position)
            && let Some(mut actual) = crate::flow::projected_leaf_place(self.program, *actual)
            && let Some(observed) = crate::flow::projected_leaf_place(self.program, left)
        {
            actual.segments.append(&mut required.segments);
            return actual == observed;
        }
        let equal = |left, right, remaining: &mut usize| {
            self.compare(left, right, formal, remaining, depth + 1)
        };
        match (table.expression(left), table.expression(right)) {
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
        crate::authored_selections::typed_operator_has_no_authored_selection(
            self.program,
            expression,
        )
    }

    fn captured(
        &self,
        expression: ExpressionHandle,
        remaining: &mut usize,
    ) -> Option<ExpressionHandle> {
        let Some(path) = self.program.expression_table.name_path_node(expression) else {
            return None;
        };
        if path.symbol != path.head_symbol
            || self
                .program
                .expression_table
                .name_path_members(path.members)
                .len()
                != 1
        {
            return None;
        }
        let (machine, state) = validation::find_state(self.program, self.caller_state_symbol)?;
        let statements = self
            .program
            .statement_table
            .statements(state.statement_nodes);
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
        let mut roots = vec![local.symbol];
        self.read_roots(local.initial_value, &mut roots, remaining, 0)?;
        let mut names = Vec::new();
        for root in roots {
            let name = self.program.symbols.name(root);
            let declarations = self
                .program
                .state_parameters(state)
                .iter()
                .filter(|p| p.name.as_str() == name)
                .map(|p| p.symbol)
                .chain(statements.iter().filter_map(|node| match node {
                    StatementNode::LocalData(l) if l.name.as_str() == name => Some(l.symbol),
                    _ => None,
                }))
                .collect::<Vec<_>>();
            if declarations.as_slice() != [root] {
                return None;
            }
            names.push(name);
        }
        let frames = self.call_frames?;
        let preserves = |frame: facts::NormalizedWriteFrame| {
            frame.into_complete_paths().is_some_and(|paths| {
                paths.iter().all(|written| {
                    names
                        .iter()
                        .all(|name| !validation::frame_paths_overlap(written, name))
                })
            })
        };
        for node in statements.get(definition + 1..self.statement_index)? {
            *remaining = remaining.checked_sub(1)?;
            if !preserves(frames.statement_value_write_frame(machine, node)) {
                return None;
            }
            match node {
                StatementNode::Assignment(_)
                    if !preserves(frames.assignment_write_frame(machine, node)) =>
                {
                    return None;
                }
                StatementNode::Call(call) if !preserves(frames.may_write_frame(machine, call)) => {
                    return None;
                }
                StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => return None,
                _ => {}
            }
        }
        // Arrival arguments can write before the receiving call begins.
        for argument in self.arguments {
            if !preserves(frames.expression_write_frame(machine, *argument)) {
                return None;
            }
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

    fn read_roots(
        &self,
        expression: ExpressionHandle,
        roots: &mut Vec<SymbolHandle>,
        remaining: &mut usize,
        depth: usize,
    ) -> Option<()> {
        if depth >= 64 || *remaining == 0 {
            return None;
        }
        *remaining -= 1;
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                let names = self
                    .program
                    .expression_table
                    .name_path_members(path.members);
                let symbols = self
                    .program
                    .expression_table
                    .name_path_member_symbols(path.member_symbols);
                if !path.head_symbol.is_valid()
                    || !path.symbol.is_valid()
                    || (names.len() == 1 && path.symbol != path.head_symbol)
                    || (names.len() > 1
                        && (names.len() != symbols.len()
                            || symbols.first() != Some(&path.head_symbol)
                            || symbols.last() != Some(&path.symbol)
                            || !symbols.iter().zip(names).all(|(symbol, name)| {
                                symbol.is_valid()
                                    && self.program.symbols.name(*symbol) == name.as_str()
                            })))
                {
                    return None;
                }
                crate::flow::projected_leaf_place(self.program, expression)?;
                if !roots.contains(&path.head_symbol) {
                    roots.push(path.head_symbol);
                }
            }
            ExpressionNode::Member(member) => {
                self.read_roots(member.receiver, roots, remaining, depth + 1)?
            }
            ExpressionNode::Indexed(indexed) => {
                self.read_roots(indexed.collection, roots, remaining, depth + 1)?;
                self.read_roots(indexed.index, roots, remaining, depth + 1)?;
            }
            ExpressionNode::Borrow(borrow) => {
                self.read_roots(borrow.target, roots, remaining, depth + 1)?
            }
            ExpressionNode::Unary(unary) if self.builtin(expression) => {
                self.read_roots(unary.operand, roots, remaining, depth + 1)?
            }
            ExpressionNode::Binary(binary)
                if self.same_binary_meaning(expression, expression, false) =>
            {
                self.read_roots(binary.left, roots, remaining, depth + 1)?;
                self.read_roots(binary.right, roots, remaining, depth + 1)?;
            }
            ExpressionNode::Call(_) => {
                let (_, operand) =
                    typed_trees::byte_predicates::byte_predicate_call(self.program, expression)?;
                self.read_roots(operand, roots, remaining, depth + 1)?;
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::String(_) => {}
            _ => return None,
        }
        Some(())
    }
}

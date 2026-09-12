//! A symbolic snapshot names the last selected store before its read, not the
//! contents of that place at exit. Selected RHS custody supplies the value;
//! complete alias-aware write frames establish the definition-to-read interval.
//! Recursive reads use the store's pre-assignment coordinate, so self-updates
//! capture the previous value. Unknown writes stop the search rather than
//! exposing an older initializer. No callee body supplies a result equation.

use super::{
    CheckedBooleanExpression, CheckedScalarExpressionRole, ExitScalars, PrimitiveType, SymbolHandle,
};
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

impl ExitScalars<'_, '_> {
    pub(super) fn bind_boolean_storage_at(
        &self,
        symbol: SymbolHandle,
        before_statement: u32,
        entry_position: &dyn Fn(SymbolHandle) -> Option<usize>,
        remaining: &mut usize,
        depth: usize,
    ) -> Option<CheckedBooleanExpression> {
        if depth >= 64 || *remaining == 0 || !symbol.is_valid() {
            return None;
        }
        *remaining -= 1;
        let state = crate::find_state_in_machine(
            self.program,
            self.machine.symbol,
            self.exit.state_symbol,
        )?;
        let statements = self
            .program
            .statement_table
            .statements(state.statement_nodes);
        let before = usize::try_from(before_statement).ok()?;
        *remaining = remaining.checked_sub(statements.len())?;
        let mut declarations = statements.get(..before)?.iter().filter_map(|statement| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            (local.symbol == symbol).then_some(local)
        });
        let local = declarations.next()?;
        if declarations.next().is_some()
            || !local.is_mutable
            || self.program.primitive_type_reference(local.type_reference)
                != Some(PrimitiveType::Bool)
            || self.program.symbols.name(symbol) != local.name.as_str()
            || self.program.symbols.get(symbol).kind != symbols::SymbolKind::Local
            || self.program.symbols.get(symbol).parent != state.symbol
        {
            return None;
        }
        // The existing frame API uses source paths. Join its root to the exact
        // declaration first; ambiguous spelling cannot identify storage.
        if statements
            .iter()
            .filter(|statement| {
                matches!(statement,
            StatementNode::LocalData(candidate) if candidate.name == local.name)
            })
            .count()
            != 1
            || self
                .program
                .state_parameters(state)
                .iter()
                .any(|parameter| parameter.name == local.name)
        {
            return None;
        }
        let frames = self.call_frames?;
        let preserves = |frame: facts::NormalizedWriteFrame| {
            frame.into_complete_paths().is_some_and(|paths| {
                paths
                    .iter()
                    .all(|written| !validation::frame_paths_overlap(written, local.name.as_str()))
            })
        };
        // A statement ordinal has no operand-prefix coordinate. Until such
        // evidence is available, a same-statement call that may change this
        // place prevents every read from being treated as a pre-statement read.
        if !preserves(frames.statement_value_write_frame(self.machine, statements.get(before)?)) {
            return None;
        }
        for (statement, node) in statements[..before].iter().enumerate().rev() {
            if *remaining == 0 {
                return None;
            }
            *remaining -= 1;
            if !preserves(frames.statement_value_write_frame(self.machine, node)) {
                return None;
            }
            let definition = match node {
                StatementNode::LocalData(candidate) if candidate.symbol == symbol => Some((
                    candidate.initial_value,
                    CheckedScalarExpressionRole::StorageInitializer,
                )),
                StatementNode::Assignment(assignment)
                    if matches!(
                        self.program.expression_table.expression(assignment.target),
                        ExpressionNode::Name(path) if path.symbol == symbol && path.head_symbol == symbol
                            && matches!(self.program.expression_table.name_path_members(path.members), [name] if name == &local.name)
                    ) =>
                {
                    Some((
                        assignment.value,
                        CheckedScalarExpressionRole::AssignmentValue,
                    ))
                }
                _ => None,
            };
            if let Some((source, role)) = definition {
                let statement = u32::try_from(statement).ok()?;
                let mut bindings = self
                    .facts
                    .values
                    .scalar_expressions
                    .source_bindings
                    .iter()
                    .filter(|(_, binding)| {
                        binding.state == self.exit.state_symbol
                            && binding.statement_ordinal == statement
                            && binding.role == role
                    });
                let (_, binding) = bindings.next()?;
                if bindings.next().is_some()
                    || binding.destination != symbol
                    || binding.expression != source
                {
                    return None;
                }
                return self.bind_boolean_expression_at(
                    statement,
                    role,
                    source,
                    entry_position,
                    remaining,
                    depth,
                );
            }
            let preserved = match node {
                StatementNode::Assignment(_) => {
                    preserves(frames.assignment_write_frame(self.machine, node))
                }
                StatementNode::Call(call) => preserves(frames.may_write_frame(self.machine, call)),
                StatementNode::AssemblyFact(_) => false,
                _ => true,
            };
            if !preserved {
                return None;
            }
        }
        None
    }

    pub(super) fn boolean_local_preserved(
        &self,
        symbol: SymbolHandle,
        definition: u32,
        before_statement: u32,
        remaining: &mut usize,
    ) -> Option<()> {
        let state = crate::find_state_in_machine(
            self.program,
            self.machine.symbol,
            self.exit.state_symbol,
        )?;
        let statements = self
            .program
            .statement_table
            .statements(state.statement_nodes);
        *remaining = remaining.checked_sub(statements.len())?;
        let definition = usize::try_from(definition).ok()?;
        let before = usize::try_from(before_statement).ok()?;
        let StatementNode::LocalData(local) = statements.get(definition)? else {
            return None;
        };
        if definition >= before || local.symbol != symbol
            || self.program.symbols.get(symbol).kind != symbols::SymbolKind::Local
            || self.program.symbols.get(symbol).parent != state.symbol
            || self.program.symbols.name(symbol) != local.name.as_str()
            || statements.iter().filter(|node| matches!(node, StatementNode::LocalData(candidate) if candidate.name == local.name)).count() != 1
            || self.program.state_parameters(state).iter().any(|parameter| parameter.name == local.name)
        {
            return None;
        }
        let frames = self.call_frames?;
        let preserves = |frame: facts::NormalizedWriteFrame| {
            frame.into_complete_paths().is_some_and(|paths| {
                paths
                    .iter()
                    .all(|written| !validation::frame_paths_overlap(written, local.name.as_str()))
            })
        };
        // `let` fixes a binding, not its bytes: a mutable loan can overwrite
        // that storage. Preserve the initializer only up to this exact capture,
        // including prior operand effects but excluding the receiving call.
        for node in statements.get(definition + 1..before)? {
            *remaining = remaining.checked_sub(1)?;
            if !preserves(frames.statement_value_write_frame(self.machine, node)) {
                return None;
            }
            let preserved = match node {
                StatementNode::Assignment(_) => {
                    preserves(frames.assignment_write_frame(self.machine, node))
                }
                StatementNode::Call(call) => preserves(frames.may_write_frame(self.machine, call)),
                StatementNode::AssemblyFact(_) => false,
                _ => true,
            };
            if !preserved {
                return None;
            }
        }
        preserves(frames.statement_value_write_frame(self.machine, statements.get(before)?))
            .then_some(())
    }
}

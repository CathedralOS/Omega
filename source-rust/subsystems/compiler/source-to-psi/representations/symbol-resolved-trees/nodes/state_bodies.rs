//! Executable statements and lexical declarations of one resolved state.
use crate::source_resolved::SymbolResolvedTrees;
use crate::source_resolved::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use crate::source_resolved::state::State;
use crate::source_resolved::statement::{Statement, StatementNode, TableLocalData};
use crate::source_resolved::types::TypeReference;
use arena::{Arena, HandleSpan};
use symbols::SymbolHandle;

impl SymbolResolvedTrees {
    /// The executable statements published by resolution, in evaluation order.
    pub fn state_body(&self, state: &State) -> &[StatementNode] {
        self.statement_table().statements(state.statement_nodes)
    }

    /// Local instructions and their declared types, including lexical operand owners.
    pub fn state_locals(
        &self,
        state: &State,
    ) -> impl Iterator<Item = (&TableLocalData, &TypeReference)> {
        self.state_body(state)
            .iter()
            .zip(self.construction_statements(state.storage.construction_statements))
            .filter_map(|(instruction, declaration)| declared_local(instruction, declaration))
    }

    /// A local of the state's body, or of a block value inside it.
    pub fn state_local(
        &self,
        state: &State,
        symbol: SymbolHandle,
    ) -> Option<(&TableLocalData, &TypeReference)> {
        if !symbol.is_valid() {
            return None;
        }
        let declarations = self.construction_statements(state.storage.construction_statements);
        if let Some(position) = self
            .resolution_indexes
            .local_statement_position(declarations, symbol)
        {
            let (local, declared_type) = declared_local(
                self.state_body(state).get(position as usize)?,
                declarations.get(position as usize)?,
            )?;
            return (local.symbol == symbol).then_some((local, declared_type));
        }
        // Block values publish their statements and declarations as two
        // spans at the same ordinals, as the body does.
        let expressions = &self.tables.bodies.expressions;
        let mut pending = declarations.iter().collect::<Vec<_>>();
        while let Some(statement) = pending.pop() {
            for block in statement_blocks(expressions, statement) {
                let ExpressionNode::Block(block) = expressions.expression(block) else {
                    continue;
                };
                let nodes = self.statement_table().statements(block.statement_nodes);
                let declared = self.construction_statements(block.statements);
                if let Some(found) = nodes
                    .iter()
                    .zip(declared)
                    .filter_map(|(instruction, declaration)| {
                        declared_local(instruction, declaration)
                    })
                    .find(|(local, _)| local.symbol == symbol)
                {
                    return Some(found);
                }
                pending.extend(declared);
            }
        }
        None
    }

    /// The statements of every block value a statement holds, including
    /// blocks in another block's final value but not those inside another
    /// block's statements: a caller that visits each returned span's
    /// statements reaches those through them.
    pub fn statement_block_spans(&self, statement: &Statement) -> Vec<HandleSpan<Statement>> {
        statement_block_spans(&self.tables.bodies.expressions, statement)
    }
}

/// The block values a statement evaluates directly, outermost first: blocks
/// in its operands, and blocks in those blocks' final values. Statements
/// inside a block belong to that block's own scope and are not entered.
pub fn statement_blocks(
    expressions: &ExpressionTable,
    statement: &Statement,
) -> Vec<ExpressionHandle> {
    let mut operands = Vec::new();
    statement.visit_operands(expressions, |operand| operands.push(operand));
    let mut blocks = Vec::new();
    for operand in operands {
        expressions.direct_blocks(operand, &mut blocks);
    }
    let mut position = 0;
    while let Some(&block) = blocks.get(position) {
        if let ExpressionNode::Block(found) = expressions.expression(block) {
            expressions.direct_blocks(found.tail, &mut blocks);
        }
        position += 1;
    }
    blocks
}

/// The statement spans of `statement_blocks`.
pub fn statement_block_spans(
    expressions: &ExpressionTable,
    statement: &Statement,
) -> Vec<HandleSpan<Statement>> {
    statement_blocks(expressions, statement)
        .into_iter()
        .filter_map(|block| match expressions.expression(block) {
            ExpressionNode::Block(block) => Some(block.statements),
            _ => None,
        })
        .collect()
}

/// Every block statement span beneath `statements`, at any depth, in the
/// order a walk that enters each block's statements after its own
/// statement reaches them. For a walk whose result does not depend on scope.
pub fn nested_block_statement_spans(
    expressions: &ExpressionTable,
    arena: &Arena<Statement>,
    statements: &[Statement],
) -> Vec<HandleSpan<Statement>> {
    let mut spans = Vec::new();
    for statement in statements {
        for span in statement_block_spans(expressions, statement) {
            spans.push(span);
            spans.extend(nested_block_statement_spans(
                expressions,
                arena,
                arena.span_or_empty(span),
            ));
        }
    }
    spans
}

// Body publication projects each declaration at the same ordinal. A local's
// instruction supplies execution metadata. Its declaration retains the rich
// type tree, which can mention source-scoped operands.
fn declared_local<'a>(
    instruction: &'a StatementNode,
    declaration: &'a Statement,
) -> Option<(&'a TableLocalData, &'a TypeReference)> {
    match (instruction, declaration) {
        (StatementNode::LocalData(local), Statement::LocalData(declared))
            if local.symbol == declared.symbol =>
        {
            Some((local, &declared.type_reference))
        }
        _ => None,
    }
}

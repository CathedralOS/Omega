//! Resolve one assembly scope without flattening its instruction paths.

use super::statement_nodes::{lower_statement_expression, lower_statement_node};
use crate::resolution::lowerer::Lowerer;
use arena::{Handle, HandleSpan};
use diagnostics::Diagnostic;
use symbol_resolved_trees::statement::{AssemblyBlock, AssemblyControlFlow, AssemblyExit};
use syntax_trees::{SyntaxTrees, statement as syntax};

pub(super) fn lower(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    handle: Handle<syntax::AssemblyControlFlow>,
    statement_index: usize,
) -> Result<Handle<AssemblyControlFlow>, Diagnostic> {
    let graph = syntax_trees.statements.assembly(handle);
    let source_blocks = syntax_trees.statements.assembly_blocks(graph.blocks);
    let mut blocks = Vec::with_capacity(source_blocks.len());
    for block in source_blocks {
        let mut instructions = Vec::new();
        for handle in syntax_trees
            .statements
            .assembly_instructions(block.instructions)
        {
            instructions.extend(lower_statement_node(
                lowerer,
                syntax_trees,
                syntax_trees.statements.statement(*handle),
                statement_index + instructions.len(),
                false,
            )?);
        }
        let requires = syntax_trees
            .statements
            .assembly_expressions(block.requires)
            .iter()
            .map(|expression| lower_statement_expression(lowerer, syntax_trees, *expression))
            .collect::<Result<Vec<_>, _>>()?;
        let ensures = syntax_trees
            .statements
            .assembly_expressions(block.ensures)
            .iter()
            .map(|expression| lower_statement_expression(lowerer, syntax_trees, *expression))
            .collect::<Result<Vec<_>, _>>()?;
        let declarations = &mut lowerer.symbol_resolved_trees.tables.declarations;
        blocks.push(AssemblyBlock {
            label: crate::lowering::name::lower_name(&block.label),
            requires: declarations.assembly_expressions.insert_many(requires),
            ensures: declarations.assembly_expressions.insert_many(ensures),
            instructions: declarations.state_statements.insert_many(instructions),
            statement_nodes: HandleSpan::empty(),
            exit: match block.exit {
                syntax::AssemblyExit::Fallthrough => AssemblyExit::Fallthrough,
                syntax::AssemblyExit::EnclosingTransition => AssemblyExit::EnclosingTransition,
                syntax::AssemblyExit::Jump(_) => AssemblyExit::Jump(Handle::invalid()),
            },
        });
    }
    let declarations = &mut lowerer.symbol_resolved_trees.tables.declarations;
    let blocks = declarations.assembly_blocks.insert_many(blocks);
    let handles: Vec<_> = blocks.handles().collect();
    for (handle, source_block) in handles.iter().copied().zip(source_blocks) {
        if let syntax::AssemblyExit::Jump(label) = &source_block.exit {
            let Some(index) = source_blocks
                .iter()
                .position(|block| block.label.as_str() == label.as_str())
            else {
                return Err(Diagnostic::error(format!(
                    "assembly jump has no local label `{}`",
                    label.as_str()
                )));
            };
            declarations.assembly_blocks.get_mut(handle).exit = AssemblyExit::Jump(handles[index]);
        }
    }
    Ok(declarations
        .assembly_graphs
        .insert(AssemblyControlFlow { blocks }))
}

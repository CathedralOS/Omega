//! Type one lexical assembly graph. Instruction and assertion nodes remain
//! in the enclosing statement window; explicit block metadata retains control.

use super::lower_statement_node;
use crate::contracts::proof_facts::lower_statement_fact;
use crate::lowerer::Lowerer;
use arena::Handle;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees::statement::{
    AssemblyBlock, AssemblyControlFlow, AssemblyControlPoint, AssemblyExit, AssemblyFactKind,
    StatementNode, TableAssemblyFact,
};

pub(crate) fn lower(
    lowerer: &mut Lowerer,
    attached_data: Option<&resolved::name::DiagnosticName>,
    state: &resolved::state::State,
    handle: Handle<resolved::statement::AssemblyControlFlow>,
) -> Result<Vec<StatementNode>, Diagnostic> {
    let declarations = &lowerer.source_trees.tables.declarations;
    let graph = declarations.assembly_graphs.get(handle);
    let mut blocks = Vec::new();
    let mut body = Vec::new();
    for block in declarations.assembly_blocks.span_or_empty(graph.blocks) {
        let start = body.len();
        let requires = declarations
            .assembly_expressions
            .span_or_empty(block.requires)
            .iter()
            .map(|expression| lower_statement_fact(lowerer, *expression))
            .collect::<Result<Vec<_>, _>>()?;
        let ensures = declarations
            .assembly_expressions
            .span_or_empty(block.ensures)
            .iter()
            .map(|expression| lower_statement_fact(lowerer, *expression))
            .collect::<Result<Vec<_>, _>>()?;
        // A label is a control point, not an assertion or a cleanup boundary.
        body.push(StatementNode::Assembly(AssemblyControlPoint::Label(
            Handle::invalid(),
        )));
        body.extend(requires.into_iter().map(|fact| {
            StatementNode::AssemblyFact(TableAssemblyFact {
                kind: AssemblyFactKind::Requires,
                fact,
            })
        }));
        let mut instructions = lowerer
            .source_trees
            .tables
            .bodies
            .statements
            .statements(block.statement_nodes)
            .iter()
            .map(|instruction| {
                lower_statement_node(lowerer, attached_data, state, instruction, true)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let transfer = if matches!(
            block.exit,
            resolved::statement::AssemblyExit::EnclosingTransition
        ) {
            instructions.pop()
        } else {
            None
        };
        body.extend(instructions);
        body.extend(ensures.into_iter().map(|fact| {
            StatementNode::AssemblyFact(TableAssemblyFact {
                kind: AssemblyFactKind::Ensures,
                fact,
            })
        }));
        body.extend(transfer);
        blocks.push(AssemblyBlock {
            label: crate::lowerer::name::lower_name(&block.label),
            start: u32::try_from(start)
                .map_err(|_| Diagnostic::error("assembly statement window exceeds u32"))?,
            count: u32::try_from(body.len() - start)
                .map_err(|_| Diagnostic::error("assembly block exceeds u32"))?,
            exit: match block.exit {
                resolved::statement::AssemblyExit::Fallthrough => AssemblyExit::Fallthrough,
                resolved::statement::AssemblyExit::Jump(_) => AssemblyExit::Jump(Handle::invalid()),
                resolved::statement::AssemblyExit::EnclosingTransition => {
                    AssemblyExit::EnclosingTransition
                }
            },
        });
    }
    let table = &mut lowerer.typed_trees.statement_table;
    let blocks = table.insert_assembly_blocks(blocks);
    for (source_handle, target_handle) in graph.blocks.handles().zip(blocks.handles()) {
        body[table.assembly_block(target_handle).start as usize] =
            StatementNode::Assembly(AssemblyControlPoint::Label(target_handle));
        if let resolved::statement::AssemblyExit::Jump(target) =
            declarations.assembly_blocks.get(source_handle).exit
        {
            let Some(index) = graph.blocks.handles().position(|handle| handle == target) else {
                return Err(Diagnostic::error(
                    "assembly jump escaped its declaring graph",
                ));
            };
            table.assembly_block_mut(target_handle).exit =
                AssemblyExit::Jump(blocks.handle_at(index as u32));
        }
    }
    let statement_count = u32::try_from(body.len())
        .map_err(|_| Diagnostic::error("assembly statement window exceeds u32"))?;
    let graph = table.insert_assembly(AssemblyControlFlow {
        blocks,
        statement_count,
    });
    let mut statements = Vec::with_capacity(body.len() + 1);
    statements.push(StatementNode::Assembly(AssemblyControlPoint::Entry(graph)));
    statements.extend(body);
    Ok(statements)
}

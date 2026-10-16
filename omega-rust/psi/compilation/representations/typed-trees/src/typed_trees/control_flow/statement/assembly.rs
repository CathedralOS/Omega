//! Assembly control metadata over an ordinary lexical statement window.
//! Instructions and assertion nodes occupy the owning state's following
//! statements exactly once. Relative block ranges survive whole-window copying;
//! local edges retain arena block identity rather than label spelling.

use super::StatementTable;
use crate::name::Identifier;
use arena::{Handle, HandleSpan};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssemblyControlFlow {
    pub blocks: HandleSpan<AssemblyBlock>,
    pub statement_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssemblyControlPoint {
    Entry(Handle<AssemblyControlFlow>),
    Label(Handle<AssemblyBlock>),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssemblyBlock {
    pub label: Identifier,
    /// Relative to the first owned statement after the graph marker.
    pub start: u32,
    pub count: u32,
    pub exit: AssemblyExit,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum AssemblyExit {
    #[default]
    Fallthrough,
    Jump(Handle<AssemblyBlock>),
    EnclosingTransition,
}

impl StatementTable {
    pub fn insert_assembly(&mut self, graph: AssemblyControlFlow) -> Handle<AssemblyControlFlow> {
        self.assembly_graphs.insert(graph)
    }
    pub fn assembly(&self, handle: Handle<AssemblyControlFlow>) -> &AssemblyControlFlow {
        self.assembly_graphs.get(handle)
    }
    pub fn insert_assembly_blocks(
        &mut self,
        blocks: impl IntoIterator<Item = AssemblyBlock>,
    ) -> HandleSpan<AssemblyBlock> {
        self.assembly_blocks.insert_many(blocks)
    }
    pub fn assembly_blocks(&self, span: HandleSpan<AssemblyBlock>) -> &[AssemblyBlock] {
        self.assembly_blocks.span_or_empty(span)
    }
    pub fn assembly_block_mut(&mut self, handle: Handle<AssemblyBlock>) -> &mut AssemblyBlock {
        self.assembly_blocks.get_mut(handle)
    }
    pub fn assembly_block(&self, handle: Handle<AssemblyBlock>) -> &AssemblyBlock {
        self.assembly_blocks.get(handle)
    }
    pub(super) fn copy_assembly_from(
        &mut self,
        source: &Self,
        handle: Handle<AssemblyControlFlow>,
    ) -> Handle<AssemblyControlFlow> {
        let graph = source.assembly(handle);
        let blocks =
            self.insert_assembly_blocks(source.assembly_blocks(graph.blocks).iter().cloned());
        for handle in blocks.handles() {
            if let AssemblyExit::Jump(target) = self.assembly_blocks.get(handle).exit {
                let index = graph
                    .blocks
                    .handles()
                    .position(|handle| handle == target)
                    .expect("assembly copy retains only graph-local edges");
                self.assembly_blocks.get_mut(handle).exit =
                    AssemblyExit::Jump(blocks.handle_at(index as u32));
            }
        }
        self.insert_assembly(AssemblyControlFlow {
            blocks,
            statement_count: graph.statement_count,
        })
    }
}

//! Lexical instruction windows and their explicit assembly edges. Block
//! positions are construction coordinates, not new source binding scopes:
//! every instruction still names its original state-local values and storage.

use super::{AssemblyControlPoint, AssemblyExit, StatementNode, StatementTable};
use arena::HandleSpan;

/// Invocation-local control preparation. Checking publishes the corresponding
/// flow evidence; this view neither proves an edge nor changes value custody.
pub struct StatementControlGraph {
    pub blocks: Vec<StatementControlBlock>,
}

pub struct StatementExecutionIter<'source> {
    statements: &'source [StatementNode],
    order: std::vec::IntoIter<usize>,
}

/// Invocation-local ordering, separate from each statement's stable source
/// coordinate. Zero position cells name statements outside the executed path.
pub struct StatementExecutionOrder {
    source_count: usize,
    indices: Vec<usize>,
    positions: Vec<usize>,
}

impl StatementExecutionOrder {
    pub fn indices(&self) -> &[usize] {
        &self.indices
    }
    pub fn position(&self, source: usize) -> Option<usize> {
        if source == self.source_count {
            return Some(self.indices.len());
        }
        self.positions
            .get(source)
            .copied()
            .and_then(|position| position.checked_sub(1))
    }
    pub fn precedes(&self, left: usize, right: usize) -> bool {
        self.position(left)
            .zip(self.position(right))
            .is_some_and(|(left, right)| left < right)
    }
    pub fn at_or_before(&self, left: usize, right: usize) -> bool {
        self.position(left)
            .zip(self.position(right))
            .is_some_and(|(left, right)| left <= right)
    }
    pub fn latest(&self, left: usize, right: usize) -> Option<usize> {
        Some(if self.position(left)? < self.position(right)? {
            right
        } else {
            left
        })
    }
    pub fn successor(&self, source: usize) -> Option<usize> {
        let position = self.position(source)?;
        (position < self.indices.len()).then(|| {
            self.indices
                .get(position + 1)
                .copied()
                .unwrap_or(self.source_count)
        })
    }
}

impl<'source> Iterator for StatementExecutionIter<'source> {
    type Item = (usize, &'source StatementNode);
    fn next(&mut self) -> Option<Self::Item> {
        self.order
            .next()
            .map(|index| (index, &self.statements[index]))
    }
}

pub struct StatementControlBlock {
    pub start: usize,
    pub count: usize,
    pub exit: StatementControlExit,
}

pub enum StatementControlExit {
    Continue(usize),
    /// The retained final instruction supplies its ordinary source exit.
    SourceExit,
    End,
}

impl StatementControlGraph {
    /// The catalog's currently modeled local transfers are unconditional.
    /// Acyclic transfers therefore have one exact instruction order, without
    /// moving source bindings or changing their lexical cleanup boundary.
    /// Cyclic graphs remain explicit: no finite order claims to execute them.
    pub fn finite_execution_order(&self, statements: &[StatementNode]) -> Option<Vec<usize>> {
        let mut visited = vec![false; self.blocks.len()];
        let mut order = Vec::new();
        let mut next = 0;
        loop {
            let block = self.blocks.get(next)?;
            if visited[next] {
                return None;
            }
            visited[next] = true;
            let window = &statements[block.start..block.start + block.count];
            let reachable = super::reachable_state_statements(window);
            order.extend(block.start..block.start + reachable.len());
            if reachable.len() != window.len()
                || reachable.last().is_some_and(|node| {
                    node.as_transition().is_some_and(|transition| {
                        matches!(transition.guard, super::TransitionGuardNode::Always)
                    })
                })
            {
                return Some(order);
            }
            match block.exit {
                StatementControlExit::Continue(destination) => next = destination,
                StatementControlExit::SourceExit | StatementControlExit::End => return Some(order),
            }
        }
    }
}

impl StatementTable {
    pub fn admitted_execution_order(
        &self,
        statements: HandleSpan<StatementNode>,
    ) -> StatementExecutionOrder {
        let indices = self
            .finite_execution_order(statements)
            .expect("typed statement control shape was admitted")
            .expect("finite assembly control was admitted");
        let mut positions = vec![0; statements.len()];
        for (position, source) in indices.iter().copied().enumerate() {
            positions[source] = position + 1;
        }
        StatementExecutionOrder {
            source_count: statements.len(),
            indices,
            positions,
        }
    }

    /// Internal consumers after finite-control admission share source ordinals
    /// and one lexical storage frame, even when a local edge changes order.
    pub fn admitted_execution_statements(
        &self,
        statements: HandleSpan<StatementNode>,
    ) -> StatementExecutionIter<'_> {
        let order = self
            .finite_execution_order(statements)
            .expect("typed statement control shape was admitted")
            .expect("finite assembly control was admitted");
        StatementExecutionIter {
            statements: self.statements(statements),
            order: order.into_iter(),
        }
    }

    pub fn finite_execution_order(
        &self,
        statements: HandleSpan<StatementNode>,
    ) -> Result<Option<Vec<usize>>, String> {
        let graph = self.control_graph(statements)?;
        Ok(graph.finite_execution_order(self.statements(statements)))
    }

    pub fn control_graph(
        &self,
        statements: HandleSpan<StatementNode>,
    ) -> Result<StatementControlGraph, String> {
        let source = self.statements(statements);
        if source.len() != statements.len() {
            return Err("lexical control frame has stale statement handles".into());
        }
        let mut blocks = Vec::new();
        let mut start = 0;
        let mut cursor = 0;
        while cursor < source.len() {
            let StatementNode::Assembly(point) = &source[cursor] else {
                cursor += 1;
                continue;
            };
            let AssemblyControlPoint::Entry(handle) = point else {
                return Err("assembly label has no owning lexical control window".into());
            };
            let graph = self.assembly(*handle);
            let assembly = self.assembly_blocks(graph.blocks);
            if assembly.len() != graph.blocks.len() || assembly.is_empty() {
                return Err("assembly control frame has stale or absent blocks".into());
            }
            let body_start = cursor + 1;
            let body_end = body_start
                .checked_add(graph.statement_count as usize)
                .filter(|end| *end <= source.len())
                .ok_or("assembly instruction window escapes its lexical frame")?;
            // The header closes the preceding ordinary sequence. It has no
            // executable payload; owned instructions follow it exactly once.
            let first_block = blocks.len() + 1;
            blocks.push(StatementControlBlock {
                start,
                count: body_start - start,
                exit: StatementControlExit::Continue(first_block),
            });
            let continuation = first_block + assembly.len();
            let mut expected = 0;
            for (index, block) in assembly.iter().enumerate() {
                if block.start as usize != expected || block.count == 0 {
                    return Err("assembly blocks do not partition their instruction window".into());
                }
                expected = expected
                    .checked_add(block.count as usize)
                    .filter(|end| *end <= graph.statement_count as usize)
                    .ok_or("assembly block escapes its instruction window")?;
                let block_start = body_start + block.start as usize;
                let expected_label = graph.blocks.handle_at(index as u32);
                if !matches!(&source[block_start], StatementNode::Assembly(AssemblyControlPoint::Label(label)) if *label == expected_label)
                {
                    return Err("assembly block has no matching lexical label point".into());
                }
                if source[block_start + 1..body_start + expected]
                    .iter()
                    .any(|node| matches!(node, StatementNode::Assembly(_)))
                {
                    return Err("assembly block contains an independent control point".into());
                }
                let exit = match block.exit {
                    AssemblyExit::Fallthrough => {
                        StatementControlExit::Continue(if index + 1 == assembly.len() {
                            continuation
                        } else {
                            first_block + index + 1
                        })
                    }
                    AssemblyExit::Jump(target) => {
                        let index = graph
                            .blocks
                            .handles()
                            .position(|handle| handle == target)
                            .ok_or("assembly edge escapes its declaring graph")?;
                        StatementControlExit::Continue(first_block + index)
                    }
                    AssemblyExit::EnclosingTransition => StatementControlExit::SourceExit,
                };
                blocks.push(StatementControlBlock {
                    start: block_start,
                    count: block.count as usize,
                    exit,
                });
            }
            if expected != graph.statement_count as usize {
                return Err("assembly control leaves unowned instruction statements".into());
            }
            cursor = body_end;
            start = cursor;
        }
        blocks.push(StatementControlBlock {
            start,
            count: source.len() - start,
            exit: StatementControlExit::End,
        });
        Ok(StatementControlGraph { blocks })
    }
}

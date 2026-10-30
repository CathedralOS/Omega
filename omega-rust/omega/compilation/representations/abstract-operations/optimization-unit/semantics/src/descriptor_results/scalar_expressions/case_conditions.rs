//! Transport one evaluated case test to a conditional without replaying it.
//! Forwarding is exact across scalar bindings; a write, call, ownership change,
//! divergent observation, or cycle withholds the fact. In particular a saved
//! Boolean cannot describe a tag changed between its observation and use.

use super::*;
use std::collections::BTreeSet;
use terminal_psi::StructuralTypeDeclaration;
use terminal_semantics::descriptor_results::{DescriptorCasePredicate, DescriptorMachine};

#[cfg(test)]
mod tests;

type Position = (BlockId, usize);

#[derive(Clone, PartialEq, Eq)]
struct CapturedTest {
    observation: Position,
    predicate: DescriptorCasePredicate,
    truth: bool,
}

impl ScalarExpressions<'_> {
    pub(in crate::descriptor_results) fn retain_case_conditions(
        &self,
        types: &[StructuralTypeDeclaration],
        graph: &mut DescriptorMachine,
    ) {
        let mut conditions = Vec::new();
        for block in self.blocks.values() {
            for (index, node) in block.nodes.iter().enumerate() {
                let O::Conditional {
                    condition,
                    when_true,
                    when_false,
                } = &node.operation
                else {
                    continue;
                };
                let Some(test) = self.case_test(
                    types,
                    graph,
                    *condition,
                    (block.id, index),
                    &mut BTreeSet::new(),
                ) else {
                    continue;
                };
                let predicates = node
                    .successors
                    .iter()
                    .map(|edge| {
                        let truth = if edge.psi_edge == when_true.psi_edge
                            && edge.target == when_true.target
                        {
                            test.truth
                        } else if edge.psi_edge == when_false.psi_edge
                            && edge.target == when_false.target
                        {
                            !test.truth
                        } else {
                            return None;
                        };
                        Some((test.predicate.clone(), truth))
                    })
                    .collect::<Option<Vec<_>>>();
                if let Some(predicates) = predicates.filter(|predicates| predicates.len() == 2) {
                    if let Ok(index) = u32::try_from(index) {
                        conditions.push(((block.id, index), predicates));
                    }
                }
            }
        }
        graph.case_successors.extend(conditions);
    }

    fn case_test(
        &self,
        types: &[StructuralTypeDeclaration],
        graph: &DescriptorMachine,
        value: ValueId,
        position: Position,
        active: &mut BTreeSet<(ValueId, Position)>,
    ) -> Option<CapturedTest> {
        if !active.insert((value, position)) {
            return None;
        }
        let result = (|| {
            if let Some(definition) = self.definitions.get(&value) {
                let node = definition.node;
                let (block, index) = definition.position;
                if block != position.0
                    || index >= position.1
                    || !self.unchanged(graph, position.0, index + 1, position.1)
                {
                    return None;
                }
                return match &node.operation {
                    O::StructuralCaseMembership {
                        source, path, case, ..
                    } => {
                        let subject =
                            terminal_semantics::descriptor_lifetimes::descriptor_location(
                                types,
                                *source,
                                *graph.place_types.get(source)?,
                                path,
                                None,
                            )?;
                        Some(CapturedTest {
                            observation: (block, index),
                            predicate: DescriptorCasePredicate {
                                subject,
                                case: *case,
                            },
                            truth: true,
                        })
                    }
                    O::BooleanNot { operand, .. } => {
                        let mut test =
                            self.case_test(types, graph, *operand, (block, index), active)?;
                        test.truth = !test.truth;
                        Some(test)
                    }
                    _ => None,
                };
            }
            if self.owners.get(&value) != Some(&position.0)
                || !self.unchanged(graph, position.0, 0, position.1)
            {
                return None;
            }
            let incoming = self.incoming.get(&position.0)?;
            let mut selected = None;
            for incoming in incoming {
                // Moving a structural subject changes its storage namespace.
                // A bare scalar forwarding path cannot establish that mapping.
                if !incoming.edge.structural_bindings.is_empty() {
                    return None;
                }
                let actual = incoming.edge.binding(value)?.argument;
                let test = self.case_test(types, graph, actual, incoming.source, active)?;
                if selected.as_ref().is_some_and(|selected| selected != &test) {
                    return None;
                }
                selected = Some(test);
            }
            selected
        })();
        active.remove(&(value, position));
        result
    }

    fn unchanged(
        &self,
        graph: &DescriptorMachine,
        block: BlockId,
        start: usize,
        end: usize,
    ) -> bool {
        let Some(nodes) = self
            .blocks
            .get(&block)
            .and_then(|block| block.nodes.get(start..end))
        else {
            return false;
        };
        nodes.iter().enumerate().all(|(offset, node)| {
            let Ok(index) = u32::try_from(start + offset) else {
                return false;
            };
            let position = (block, index);
            graph.flow.get(&position).is_some_and(|step| {
                step.definitions.is_empty()
                    && step.derivations.is_empty()
                    && !step.may_escape
                    && !graph.calls.contains_key(&position)
                    && !graph.contents_transfers.contains_key(&position)
                    && !graph.payload_invalidations.contains_key(&position)
                    && node.ownership.is_empty()
            })
        })
    }
}

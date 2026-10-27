//! Inert runtime element subjects for predicate consequence questions.
//!
//! A goal may describe a read without executing it. Its private uninterpreted
//! value adds no bounds, safety decision, or storage authority. The predicate
//! interpreter still proves every actual read's bounds on its own path. Equal
//! collection/selector observations reuse the existing functional read owner;
//! every equality alternative remains in the question. Thus a guarded total
//! predicate may imply a relation on its true returns without treating its
//! truth as permission to form an earlier partial operand. Public replacement
//! separately owes the converse over all original carriers, including guards.

use super::{Frame, Implication, Integer, Path, Value};
use crate::proof_contracts::collection_predicates::{
    PredicateApplication, PredicateIntegerOperand, PredicateIntegerReadSubject,
};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(super) type Subjects = Vec<(PredicateIntegerOperand, Integer)>;

impl Implication<'_> {
    pub(super) fn runtime_read_subjects(
        &mut self,
        application: &PredicateApplication<'_>,
        frame: &Frame,
        path: Path,
        subjects: Subjects,
    ) -> Option<Vec<(Path, Subjects)>> {
        let program = self.program;
        let machine = &program.machines()[application.machine];
        let entry = self.program.machine_states(machine).first()?;
        let mut expressions = Vec::new();
        for statement in self
            .program
            .statement_table
            .statements(entry.statement_nodes)
        {
            self.spend()?;
            crate::expression_traversal::collect_statement_expression_trees(
                self.program,
                statement,
                &mut expressions,
            );
        }
        let mut reads = Vec::new();
        for expression in expressions {
            self.spend()?;
            let Some(indexed) = self.program.expression_table.indexed_node(expression) else {
                continue;
            };
            if self
                .program
                .expression_table
                .constant_integer_value(indexed.index)
                .is_some()
            {
                continue;
            }
            let Some(identity) =
                crate::immutable_entry_indexed_read_identity(self.program, expression)
            else {
                continue;
            };
            if !reads.iter().any(|(_, previous)| *previous == identity) {
                reads.push((expression, identity));
            }
        }
        let mut candidates = vec![(path, subjects)];
        for (position, (expression, _)) in reads.into_iter().enumerate() {
            let mut next = Vec::new();
            for (path, subjects) in candidates {
                for (path, value) in self.inert_read(frame, path, expression)? {
                    self.spend()?;
                    let mut subjects = subjects.clone();
                    // Put authored dynamic questions first: the bounded work
                    // belongs to relevant reads before unrelated roster pairs.
                    subjects.insert(
                        position,
                        (
                            PredicateIntegerOperand::Read(PredicateIntegerReadSubject {
                                predicate: machine.symbol,
                                expression,
                                arguments: application.arguments.to_vec(),
                            }),
                            value,
                        ),
                    );
                    next.push((path, subjects));
                }
            }
            candidates = next;
        }
        Some(candidates)
    }

    pub(super) fn inert_read(
        &mut self,
        frame: &Frame,
        path: Path,
        expression: ExpressionHandle,
    ) -> Option<Vec<(Path, Integer)>> {
        let ExpressionNode::Indexed(indexed) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        let mut values = Vec::new();
        for (path, collection) in self.expression(frame, path, indexed.collection, 0)? {
            let Value::Collection {
                identity, element, ..
            } = collection
            else {
                return None;
            };
            for (path, selector) in self.expression(frame, path, indexed.index, 0)? {
                let Value::Integer {
                    value: selector, ..
                } = selector
                else {
                    return None;
                };
                // Only the private question is totalized. This adds no length
                // premise and is never the source Indexed execution route.
                for (path, value) in self.read(path, identity, selector, element, 0)? {
                    let Value::Integer { value, .. } = value else {
                        return None;
                    };
                    values.push((path, value));
                }
            }
        }
        Some(values)
    }
}

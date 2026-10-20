//! A stored Boolean retains conditional arithmetic observations at evaluation.
//! The two polarities are consequences, not asserted facts. Selecting a guard
//! imports only its polarity under the original captured scalar roster. Writes
//! retire the Boolean's storage association, while writing an earlier numeric
//! source cannot change the historical values already captured by that Boolean.
//! Local bindings, record fields and copied records use the same transfer;
//! unknown producers contribute no implication.

use super::{CurrentScalarValues, field_storage_path};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::PrimitiveType;
use validation::ScopedArithmeticHypothesis;

#[derive(Clone)]
pub(super) struct ConditionBinding {
    pub(super) owner: SymbolHandle,
    pub(super) path: Vec<SymbolHandle>,
    pub(super) name: String,
    when_true: Vec<ScopedArithmeticHypothesis>,
    when_false: Vec<ScopedArithmeticHypothesis>,
}

impl ConditionBinding {
    pub(super) fn hypotheses(&self, holds: bool) -> &[ScopedArithmeticHypothesis] {
        if holds {
            &self.when_true
        } else {
            &self.when_false
        }
    }
}

pub(super) struct CapturedCondition {
    path: Vec<SymbolHandle>,
    when_true: Vec<ScopedArithmeticHypothesis>,
    when_false: Vec<ScopedArithmeticHypothesis>,
}

impl CurrentScalarValues<'_> {
    pub(super) fn capture_conditions(
        &self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Vec<CapturedCondition> {
        if depth >= 64 {
            return Vec::new();
        }
        let reference = validation::expression_result_type_reference(
            self.program,
            self.machine,
            self.state,
            expression,
        );
        if reference.and_then(|reference| self.program.primitive_type_reference(reference))
            == Some(PrimitiveType::Bool)
        {
            let capture = |holds| {
                self.capture_condition(
                    self.program,
                    self.machine,
                    self.state,
                    expression,
                    holds,
                    depth + 1,
                )
            };
            let when_true = capture(true);
            let when_false = capture(false);
            return if when_true.is_empty() && when_false.is_empty() {
                Vec::new()
            } else {
                vec![CapturedCondition {
                    path: Vec::new(),
                    when_true,
                    when_false,
                }]
            };
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::StructLiteral(literal) if literal.case_symbol.is_none() => {
                let mut captured = Vec::new();
                for field in self.program.expression_table.struct_fields(literal.fields) {
                    if !field.field_symbol.is_valid() {
                        continue;
                    }
                    for mut condition in self.capture_conditions(field.value, depth + 1) {
                        condition.path.insert(0, field.field_symbol);
                        captured.push(condition);
                    }
                }
                captured
            }
            ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
                let Some((owner, prefix)) = field_storage_path(self.program, expression) else {
                    return Vec::new();
                };
                self.conditions
                    .iter()
                    .filter(|condition| {
                        condition.owner == owner && condition.path.starts_with(&prefix)
                    })
                    .map(|condition| CapturedCondition {
                        path: condition.path[prefix.len()..].to_vec(),
                        when_true: condition.when_true.clone(),
                        when_false: condition.when_false.clone(),
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    pub(super) fn replace_conditions(
        &mut self,
        owner: SymbolHandle,
        prefix: &[SymbolHandle],
        conditions: Vec<CapturedCondition>,
    ) {
        self.conditions.retain(|condition| {
            condition.owner != owner
                || !(condition.path.starts_with(prefix) || prefix.starts_with(&condition.path))
        });
        for condition in conditions {
            let mut path = prefix.to_vec();
            path.extend(condition.path);
            let mut name = self.program.symbols.name(owner).to_owned();
            for field in &path {
                name.push('.');
                name.push_str(self.program.symbols.name(*field));
            }
            self.conditions.push(ConditionBinding {
                owner,
                path,
                name,
                when_true: condition.when_true,
                when_false: condition.when_false,
            });
        }
    }
}

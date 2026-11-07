//! Selected Unit completions use ordinary evaluated dispatch operands and exact
//! call occurrences. They create no value carrier or dummy result storage.

use super::{Builder, ExpressionHandle, ExpressionNode};
use checked_trees::{
    CheckedCompletionArm, CheckedCompletionExpression, CheckedCompletionExpressionHandle,
    CheckedCompletionExpressionKind as Kind, CheckedCompletionPlans, CheckedCompletionRefusal,
    CheckedCompletionRefusalReason as Refusal, CheckedCompletionRoot,
};
use std::collections::BTreeMap;

impl Builder<'_, '_> {
    pub(super) fn record_completion(
        &mut self,
        expression: ExpressionHandle,
        plans: &mut CheckedCompletionPlans,
    ) -> Option<()> {
        if !matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Match(_)
        ) {
            return None;
        }
        let (root, refusal) = match self.completion_node(expression, plans, &mut BTreeMap::new()) {
            Ok(root) => (root, None),
            Err(refusal) => (CheckedCompletionExpressionHandle::invalid(), Some(refusal)),
        };
        plans.roots.push(CheckedCompletionRoot {
            machine: self.machine,
            state: self.state,
            statement_ordinal: u32::try_from(self.statement_index).ok()?,
            expression,
            root,
            refusal,
        });
        refusal.is_none().then_some(())
    }

    fn completion_node(
        &mut self,
        expression: ExpressionHandle,
        plans: &mut CheckedCompletionPlans,
        visited: &mut BTreeMap<(u32, u32), Option<CheckedCompletionExpressionHandle>>,
    ) -> Result<CheckedCompletionExpressionHandle, CheckedCompletionRefusal> {
        let declined = |reason| CheckedCompletionRefusal { expression, reason };
        let coordinate = (expression.arena_index(), expression.generation());
        if let Some(previous) = visited.get(&coordinate) {
            return previous.ok_or_else(|| declined(Refusal::Cycle));
        }
        visited.insert(coordinate, None);
        let kind = match self.program.expression_table.expression(expression).clone() {
            ExpressionNode::Call(call) => {
                let machine = self
                    .program
                    .machine_by_symbol(self.machine)
                    .ok_or_else(|| declined(Refusal::CallResult))?;
                if !validation::call_returns_unit(self.program, machine, expression) {
                    return Err(declined(Refusal::CallResult));
                }
                let target = validation::call_target_symbol(self.program, &call);
                let (source_call, _) = self
                    .call_ordinal(expression, target)
                    .ok_or_else(|| declined(Refusal::CallOccurrence))?;
                Kind::Invocation { source_call }
            }
            ExpressionNode::Match(dispatch) => {
                let (subject, prepared) = self
                    .prepare_dispatch(expression, &dispatch)
                    .ok_or_else(|| declined(Refusal::DispatchOperands))?;
                let mut arms = Vec::with_capacity(prepared.len());
                for arm in prepared {
                    let comparison = match (&subject, &arm.pattern) {
                        (
                            checked_trees::CheckedDispatchSubject::Scalar(subject),
                            checked_trees::CheckedScalarDispatchPattern::Value(_),
                        ) => {
                            let primitive = self.plans.nodes.get(*subject).primitive_type;
                            if primitive == super::PrimitiveType::Bool {
                                checked_trees::CheckedCompletionComparison::Boolean
                            } else if primitive.is_fixed_integer()
                                || primitive == super::PrimitiveType::Addr
                            {
                                checked_trees::CheckedCompletionComparison::Integer
                            } else {
                                return Err(declined(Refusal::Comparison));
                            }
                        }
                        (
                            _,
                            checked_trees::CheckedScalarDispatchPattern::Wildcard
                            | checked_trees::CheckedScalarDispatchPattern::Predicate(_),
                        ) => checked_trees::CheckedCompletionComparison::Predicate,
                        _ => return Err(declined(Refusal::Comparison)),
                    };
                    arms.push(CheckedCompletionArm {
                        source_arm: arm.source_arm,
                        equality_use: arm.equality_use,
                        comparison,
                        pattern: arm.pattern,
                        completion: self.completion_node(arm.value, plans, visited)?,
                    });
                }
                Kind::Dispatch {
                    subject,
                    arms: plans.arms.insert_many(arms),
                }
            }
            _ => return Err(declined(Refusal::ExpressionKind)),
        };
        let normal_continuation = match &kind {
            Kind::Invocation { .. } => {
                validation::expression_normal_completion_possible(self.program, expression)
                    .ok_or_else(|| declined(Refusal::NormalCompletion))?
            }
            Kind::Dispatch { arms, .. } => plans
                .arms
                .span(*arms)
                .ok_or_else(|| declined(Refusal::DispatchOperands))?
                .iter()
                .any(|arm| plans.nodes.get(arm.completion).normal_continuation),
            Kind::Missing => return Err(declined(Refusal::ExpressionKind)),
        };
        let value = plans.nodes.append(CheckedCompletionExpression {
            expression,
            kind,
            normal_continuation,
        });
        visited.insert(coordinate, Some(value));
        Ok(value)
    }
}

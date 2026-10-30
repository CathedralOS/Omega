//! Crash possibility reads the entry-path receipt produced by evaluation.
//! These historical conditions are independent of current-storage facts and
//! the selecting caller. Unknown observations widen a route, never erase it.

use super::summary_predicates::{SummaryCrashPredicate, SummaryCrashRouteGuard};
use crate::flow::active_context_handles;
use checked_trees::{
    CheckedBooleanExpression as Boolean, CheckedScalarExpression as Scalar,
    CheckedStructuralPredicatePathSegment as Path, CrashPredicateExpression as Expression,
};
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, UnaryOperator};

pub(super) struct EntryConditions<'a> {
    program: &'a dyn validation::CheckingInput,
    semantic: &'a facts::FactPlan,
    flow: &'a checked_trees::FlowFacts,
}

impl<'a> EntryConditions<'a> {
    pub(super) fn new(
        program: &'a dyn validation::CheckingInput,
        semantic: &'a facts::FactPlan,
        flow: &'a checked_trees::FlowFacts,
    ) -> Self {
        Self {
            program,
            semantic,
            flow,
        }
    }

    pub(super) fn for_machine(&self, machine: SymbolHandle) -> MachineEntryConditions<'_> {
        let scalars = self
            .program
            .machine_by_symbol(machine)
            .and_then(|machine| self.program.machine_states(machine).first())
            .and_then(|entry| {
                self.program
                    .state_parameters(entry)
                    .iter()
                    .enumerate()
                    .filter(|(_, parameter)| {
                        crate::values::occupies_scalar_position(self.program, parameter)
                    })
                    .map(|(position, _)| u32::try_from(position))
                    .collect::<Result<Vec<_>, _>>()
                    .ok()
            });
        MachineEntryConditions {
            query: self,
            machine,
            scalars,
        }
    }
}

pub(super) struct MachineEntryConditions<'a> {
    query: &'a EntryConditions<'a>,
    machine: SymbolHandle,
    /// Checked scalar positions are dense; route identity uses authored positions.
    scalars: Option<Vec<u32>>,
}

impl MachineEntryConditions<'_> {
    pub(super) fn at_call(&self, call: &checked_trees::FlowCallFact) -> SummaryCrashRouteGuard {
        self.from_contexts(call.entry_semantic_contexts)
    }

    pub(super) fn at_site(
        &self,
        location: checked_trees::CrashSiteLocation,
    ) -> SummaryCrashRouteGuard {
        self.query
            .flow
            .control
            .state(self.machine, location.state())
            .and_then(|state| {
                self.query
                    .flow
                    .control
                    .statements
                    .span_or_empty(state.statements)
                    .iter()
                    .find(|statement| {
                        statement.statement_index == location.statement_ordinal() as usize
                    })
            })
            .map_or(SummaryCrashRouteGuard::Truth, |statement| {
                self.from_contexts(statement.entry_semantic_contexts)
            })
    }

    fn from_contexts(
        &self,
        contexts: arena::HandleSpan<checked_trees::FlowSemanticContextRef>,
    ) -> SummaryCrashRouteGuard {
        if self.scalars.is_none() {
            return SummaryCrashRouteGuard::Truth;
        }
        let contexts = active_context_handles(&self.query.flow.contexts, contexts);
        let Some(condition) = crate::facts::boolean_predicates::entry_path_necessary_condition(
            self.query.semantic,
            &contexts,
            self.machine,
        ) else {
            return SummaryCrashRouteGuard::Truth;
        };
        let Some(identity) = self.boolean(&condition, 0) else {
            return SummaryCrashRouteGuard::Truth;
        };
        if condition == Boolean::Constant(true) {
            return SummaryCrashRouteGuard::Truth;
        }
        SummaryCrashRouteGuard::Predicate(SummaryCrashPredicate {
            identity,
            builtin_meaning: true,
            scalar: Some(condition),
        })
    }

    /// Only the checked executable comparison algebra has this identity
    /// projection. Unsupported or effectful scalar forms lose precision rather
    /// than masquerading as domain-free builtin source arithmetic.
    fn boolean(&self, condition: &Boolean, depth: usize) -> Option<Expression> {
        if depth >= 64 {
            return None;
        }
        let binary =
            |operator: BinaryOperator, left: Expression, right: Expression| Expression::Binary {
                operator: operator as u8,
                left: Box::new(left),
                right: Box::new(right),
            };
        Some(match condition {
            Boolean::Constant(value) => Expression::Boolean(*value),
            Boolean::Parameter { position } => {
                Expression::Parameter(*self.scalars.as_ref()?.get(*position)?)
            }
            Boolean::StructuralParameterField {
                parameter_position,
                path,
                ..
            } => self.project(*parameter_position, path)?,
            Boolean::Not(value) => Expression::Unary {
                operator: UnaryOperator::LogicalNot as u8,
                operand: Box::new(self.boolean(value, depth + 1)?),
            },
            Boolean::And { left, right } => binary(
                BinaryOperator::And,
                self.boolean(left, depth + 1)?,
                self.boolean(right, depth + 1)?,
            ),
            Boolean::Or { left, right } => binary(
                BinaryOperator::Or,
                self.boolean(left, depth + 1)?,
                self.boolean(right, depth + 1)?,
            ),
            Boolean::Equal { left, right } => binary(
                BinaryOperator::Equal,
                self.boolean(left, depth + 1)?,
                self.boolean(right, depth + 1)?,
            ),
            Boolean::IntegerComparison { kind, left, right } => binary(
                match kind {
                    checked_trees::CheckedIntegerComparisonKind::Equal => BinaryOperator::Equal,
                    checked_trees::CheckedIntegerComparisonKind::LessThan => BinaryOperator::Less,
                    checked_trees::CheckedIntegerComparisonKind::LessOrEqual => {
                        BinaryOperator::LessOrEqual
                    }
                },
                self.scalar(left, depth + 1)?,
                self.scalar(right, depth + 1)?,
            ),
            _ => return None,
        })
    }

    fn scalar(&self, value: &Scalar, depth: usize) -> Option<Expression> {
        if depth >= 64 {
            return None;
        }
        match value {
            Scalar::Parameter { position, .. } => Some(Expression::Parameter(
                *self.scalars.as_ref()?.get(*position)?,
            )),
            Scalar::StructuralParameterField {
                parameter_position,
                path,
                ..
            } => self.project(*parameter_position, path),
            Scalar::IntegerLiteral { literal } => Some(Expression::Integer(
                literal.decimal_value_bignum()?.to_string(),
            )),
            Scalar::Boolean(condition) => self.boolean(condition, depth + 1),
            _ => None,
        }
    }

    fn project(&self, parameter: u32, path: &[Path]) -> Option<Expression> {
        let mut expression = Expression::Parameter(parameter);
        for step in path {
            expression = match step {
                Path::Field(member) if member.is_valid() => Expression::Member {
                    receiver: Box::new(expression),
                    member: member.spelling(),
                },
                Path::FixedIndex(index) => Expression::Indexed {
                    collection: Box::new(expression),
                    index: Box::new(Expression::Integer(index.to_string())),
                },
                _ => return None,
            };
        }
        Some(expression)
    }
}

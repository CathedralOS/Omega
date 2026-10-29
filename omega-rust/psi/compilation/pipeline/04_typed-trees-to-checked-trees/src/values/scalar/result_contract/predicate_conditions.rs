//! Complete predicate conditions use the ordinary typed scalar read owner.
//! The proof query owns equivalence and guard-before-read formation; this
//! adapter owns exact actual storage, declared types and selector substitution.
use super::*;
use checked_trees::{CheckedScalarReadProjection, CheckedStorageRoot};

impl ContractPredicates<'_, '_> {
    pub(super) fn exact_predicate_condition(
        &mut self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<CheckedBooleanExpression> {
        if self.allow_result || depth >= 64 {
            return None;
        }
        let conditions =
            validation::exact_predicate_conjunction(self.program, self.owner, expression);
        trace(
            "conditions",
            &(
                self.program.expression_table.display_name(expression),
                conditions.as_ref().map(Vec::len),
            ),
        );
        let conditions = conditions?;
        self.remaining
            .set(self.remaining.get().checked_sub(conditions.len())?);
        let mut predicates = Vec::new();
        for condition in conditions {
            let predicate = match condition {
                validation::PredicateCondition::Boolean { subject, holds } => {
                    let value = if subject.projection.is_empty() {
                        self.boolean(subject.expression, depth + 1)?
                    } else {
                        let (CheckedScalarExpression::Boolean(value), _) = self
                            .projected_scalar_subject(
                                subject.expression,
                                &subject.projection,
                                depth + 1,
                            )?
                        else {
                            return None;
                        };
                        *value
                    };
                    if holds {
                        value
                    } else {
                        CheckedBooleanExpression::Not(Box::new(value))
                    }
                }
                validation::PredicateCondition::Ordering(condition) => {
                    let left = self.condition_operand(&condition.left, None, depth + 1);
                    let right = self.condition_operand(
                        &condition.right,
                        left.as_ref().and_then(|term| scalar_expression_type(term)),
                        depth + 1,
                    )?;
                    let left = left.or_else(|| {
                        self.condition_operand(
                            &condition.left,
                            scalar_expression_type(&right),
                            depth + 1,
                        )
                    })?;
                    construct_integer_comparison(condition.operator, left, right)?
                }
            };
            predicates.push(predicate);
        }
        let mut predicates = predicates.into_iter();
        let first = predicates.next()?;
        Some(
            predicates.fold(first, |left, right| CheckedBooleanExpression::And {
                left: Box::new(left),
                right: Box::new(right),
            }),
        )
    }

    fn condition_operand(
        &self,
        operand: &validation::PredicateIntegerOperand,
        context: Option<PrimitiveType>,
        depth: usize,
    ) -> Option<CheckedScalarExpression> {
        if depth >= 64 || self.remaining.get() == 0 {
            return None;
        }
        self.remaining.set(self.remaining.get() - 1);
        match operand {
            validation::PredicateIntegerOperand::Argument(subject) => self
                .numeric_subject(subject.clone(), depth + 1)
                .map(|value| value.0),
            validation::PredicateIntegerOperand::Integer(value) => {
                Some(CheckedScalarExpression::IntegerLiteral {
                    literal: validation::land_integer_value(
                        &numerics::bignum::BigInt::from_i128(*value),
                        context?,
                    )?,
                })
            }
            validation::PredicateIntegerOperand::Read(read) => self.predicate_read(read, depth + 1),
        }
    }

    fn predicate_read(
        &self,
        read: &validation::PredicateIntegerReadSubject,
        depth: usize,
    ) -> Option<CheckedScalarExpression> {
        if self.allow_result || depth >= 64 {
            return None;
        }
        let machine = self.program.machine(read.predicate)?;
        let entry = self.program.machine_states(machine).first()?;
        let parameters = self.program.state_parameters(entry);
        if parameters.len() != read.arguments.len() {
            return None;
        }
        let actuals = parameters
            .iter()
            .zip(&read.arguments)
            .map(|(parameter, actual)| {
                let actual_value = self.predicate_actual_storage(*actual);
                trace(
                    "actual",
                    &(
                        self.program.expression_table.display_name(*actual),
                        &actual_value,
                    ),
                );
                let actual = actual_value?;
                let declared = validation::field_domains::readable_type_reference(
                    self.program,
                    parameter.type_reference,
                )?;
                let supplied =
                    validation::field_domains::readable_type_reference(self.program, actual.2)?;
                let matched = validation::declared_carriers_match(self.program, declared, supplied);
                trace("carriers", &(declared, supplied, matched));
                matched.then_some(actual)
            })
            .collect::<Option<Vec<_>>>()?;
        let projection_value = super::super::scalar_lowering::storage_projection(
            self.program,
            self.operators,
            read.expression,
            parameters,
            |_, selector, _| {
                let operand = validation::predicate_integer_operand(
                    self.program,
                    read.predicate,
                    &read.arguments,
                    selector,
                )?;
                self.condition_operand(&operand, Some(PrimitiveType::U64), depth + 1)
            },
        );
        trace("projection", &projection_value.is_some());
        let mut projection = projection_value?;
        let position = usize::try_from(projection.root.parameter()?).ok()?;
        let (actual_position, prefix, _) = actuals.get(position)?;
        projection.root = CheckedStorageRoot::Parameter {
            index: *actual_position,
        };
        let mut path = prefix
            .iter()
            .cloned()
            .map(CheckedScalarReadProjection::Static)
            .collect::<Vec<_>>();
        path.extend(projection.path);
        projection.path = path;
        let result = super::super::scalar_lowering::scalar_storage_read_projection(
            self.program,
            read.expression,
            self.parameters,
            projection,
        )
        .map(|value| value.0);
        trace("read-result", &result);
        result
    }
}
fn trace(stage: &str, value: &impl std::fmt::Debug) {
    if let Some(path) = std::env::var_os("OMEGA_STORAGE_PREDICATE_TRACE") {
        use std::io::Write as _;
        if let Ok(mut output) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = output.write_all(format!("ADAPTER {stage} {value:?}\n").as_bytes());
        }
    }
}

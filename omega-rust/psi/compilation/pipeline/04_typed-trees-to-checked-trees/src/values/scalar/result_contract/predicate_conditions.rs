//! Complete predicate conditions use the ordinary typed scalar read owner.
//! The proof query owns equivalence and guard-before-read formation; this
//! adapter owns exact actual storage, declared types and selector substitution.
use super::*;
use checked_trees::{CheckedScalarReadProjection, CheckedStorageRoot};

impl ContractPredicates<'_, '_> {
    pub(super) fn exact_predicate_condition(
        &self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<CheckedBooleanExpression> {
        if self.allow_result || depth >= 64 {
            return None;
        }
        let conditions =
            validation::exact_predicate_conjunction(self.program, self.owner, expression)?;
        self.remaining
            .set(self.remaining.get().checked_sub(conditions.len())?);
        let mut predicates = Vec::new();
        for condition in conditions {
            let left = self.condition_operand(&condition.left, None, depth + 1);
            let right = self.condition_operand(
                &condition.right,
                left.as_ref().and_then(|term| scalar_expression_type(term)),
                depth + 1,
            )?;
            let left = left.or_else(|| {
                self.condition_operand(&condition.left, scalar_expression_type(&right), depth + 1)
            })?;
            predicates.push(construct_integer_comparison(
                condition.operator,
                left,
                right,
            )?);
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
                let actual = self.predicate_actual_storage(*actual)?;
                let declared = validation::field_domains::readable_type_reference(
                    self.program,
                    parameter.type_reference,
                )?;
                validation::declared_carriers_match(self.program, declared, actual.2)
                    .then_some(actual)
            })
            .collect::<Option<Vec<_>>>()?;
        let mut projection = super::super::scalar_lowering::storage_projection(
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
        )?;
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
        super::super::scalar_lowering::scalar_storage_read_projection(
            self.program,
            read.expression,
            self.parameters,
            projection,
        )
        .map(|value| value.0)
    }
}

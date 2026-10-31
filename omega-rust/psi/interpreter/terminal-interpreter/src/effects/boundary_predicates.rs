//! One closed predicate evaluator for boundary input obligations and reported
//! crash guards. Callers provide only evaluated literals in the exact declared
//! input telescope; structural observations retain their actual argument and
//! original storage. The evaluator never reads ambient caller value IDs.

use proof_admission::{ClosedIntegerEvaluationError, ClosedIntegerEvaluator};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerMathTerm, IntegerValue, PlaceId, Proposition,
    PropositionContext, ScalarReadProjection, ScalarTerm, ScalarType, StructuralPlaceKind,
};
use terminal_psi::{BoundaryMachineDeclaration, StructuralArgument};

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::{TerminalScalarValue, TerminalStructuralValue};

pub(super) fn scalar_literals(arguments: &[crate::values::TerminalScalarValue]) -> Vec<ScalarTerm> {
    arguments.iter().copied().map(scalar_literal).collect()
}

fn scalar_literal(argument: TerminalScalarValue) -> ScalarTerm {
    match argument {
        crate::values::TerminalScalarValue::Boolean(value) => ScalarTerm::Boolean(value),
        crate::values::TerminalScalarValue::Integer { scalar_type, value } => {
            ScalarTerm::Integer { scalar_type, value }
        }
        crate::values::TerminalScalarValue::IeeeFloat(value) => ScalarTerm::IeeeFloatLiteral(value),
    }
}

pub(crate) fn validate_input_requirements(
    boundary: &BoundaryMachineDeclaration,
    arguments: &[crate::values::TerminalScalarValue],
    execution: &TerminalExecution<'_>,
    structural_arguments: &[StructuralArgument],
    structural_values: &[TerminalStructuralValue],
) -> Result<(), crate::errors::TerminalInterpretError> {
    use crate::errors::{TerminalInterpretError, malformed};
    if boundary.scalar_requires.is_empty() {
        return Ok(());
    }
    let arguments = scalar_literals(arguments);
    let parameters = boundary
        .scalar_contract_parameters()
        .ok_or_else(malformed)?;
    if arguments.len() != parameters.len()
        || arguments
            .iter()
            .zip(&parameters)
            .any(|(actual, parameter)| {
                actual.scalar_type() != parameter.scalar_type || actual.validate().is_err()
            })
    {
        return Err(malformed());
    }
    proof_admission::check_predicate_evaluation_size(boundary.scalar_requires.iter()).map_err(
        |_| {
            TerminalInterpretError::UnsupportedSemanticVariant(
                "boundary input requirement evaluation exceeds the resource limit",
            )
        },
    )?;
    let context = PropositionContext::from_value_types_and_places(
        parameters
            .iter()
            .map(|parameter| (parameter.id, parameter.scalar_type)),
        boundary.structural_parameters.iter().map(|parameter| {
            (
                parameter.place,
                StructuralPlaceKind::Parameter {
                    position: parameter.position,
                },
            )
        }),
    )
    .map_err(|_| malformed())?;
    let substitutions = parameters
        .iter()
        .zip(&arguments)
        .map(|(parameter, argument)| (parameter.id, argument.clone()))
        .collect();
    let mut integers = ClosedIntegerEvaluator::default();
    let observations = InputObservations {
        execution,
        boundary,
        arguments: structural_arguments,
        values: structural_values,
    };
    for requirement in &boundary.scalar_requires {
        if requirement.validate().is_err() || context.validate(requirement).is_err() {
            return Err(malformed());
        }
        let requirement = terminal_semantics::substitution::substitute_proposition_values(
            requirement,
            &substitutions,
        );
        match observations.evaluate(&requirement, &mut integers)? {
            Some(true) => {}
            Some(false) => {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "boundary input requirement is false for this invocation",
                ));
            }
            None => {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "boundary input requirement has no supported closed evaluation",
                ));
            }
        }
    }
    Ok(())
}

/// Formal places select the already resolved argument tuple. Reading a leaf
/// uses the ordinary primitive reader, including its original-home, access,
/// initialization, case, codec and bounds checks.
struct InputObservations<'execution, 'artifact, 'call> {
    execution: &'execution TerminalExecution<'artifact>,
    boundary: &'call BoundaryMachineDeclaration,
    arguments: &'call [StructuralArgument],
    values: &'call [TerminalStructuralValue],
}

impl InputObservations<'_, '_, '_> {
    fn evaluate(
        &self,
        predicate: &Proposition,
        integers: &mut ClosedIntegerEvaluator,
    ) -> Result<Option<bool>, TerminalInterpretError> {
        match predicate {
            Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                let identity = matches!(predicate, Proposition::Conjunction(_));
                for child in children {
                    match self.evaluate(child, integers)? {
                        Some(value) if value != identity => return Ok(Some(value)),
                        Some(_) => {}
                        None => return Ok(None),
                    }
                }
                Ok(Some(identity))
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => match self.evaluate(premise, integers)? {
                Some(false) => Ok(Some(true)),
                Some(true) => self.evaluate(conclusion, integers),
                None => Ok(None),
            },
            _ => {
                let mut closed = predicate.clone();
                self.close_predicate(&mut closed)?;
                closed_guard(&closed, integers).map_err(|_| {
                    TerminalInterpretError::UnsupportedSemanticVariant(
                        "boundary input requirement evaluation exceeds the resource limit",
                    )
                })
            }
        }
    }

    fn close_predicate(&self, predicate: &mut Proposition) -> Result<(), TerminalInterpretError> {
        match predicate {
            semantic_vocabulary::ordered_comparison_operands!(left, right)
            | Proposition::ScalarIeeeFloatComparison { left, right, .. } => {
                self.close_scalar(left)?;
                self.close_scalar(right)?;
            }
            semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                self.close_mathematical_integer(left)?;
                self.close_mathematical_integer(right)?;
            }
            // Other predicate families keep their existing closed evaluator;
            // an unavailable interpretation still refuses there.
            _ => {}
        }
        Ok(())
    }

    fn close_mathematical_integer(
        &self,
        term: &mut IntegerMathTerm,
    ) -> Result<(), TerminalInterpretError> {
        if let IntegerMathTerm::Embed(scalar) = term {
            self.close_scalar(scalar)?;
        } else {
            for operand in term.operands_mut() {
                self.close_mathematical_integer(operand)?;
            }
        }
        Ok(())
    }

    fn close_scalar(&self, term: &mut ScalarTerm) -> Result<(), TerminalInterpretError> {
        // The complete predicate was resource-checked before this traversal.
        // Closing children preserves the selected arithmetic and selector
        // policies; their ordinary closed-value queries compute the result.
        for operand in term.operands_mut() {
            self.close_scalar(operand)?;
        }
        let observed = match term {
            ScalarTerm::StructuralRead {
                root,
                path,
                scalar_type,
            } => {
                let mut selected = Vec::with_capacity(path.len());
                for segment in path {
                    selected.push(match segment {
                        ScalarReadProjection::Static(segment) => *segment,
                        ScalarReadProjection::Referent => CanonicalStructuralPathSegment::Referent,
                        ScalarReadProjection::Index(index) => {
                            let (_, value) = index.integer_value().ok_or_else(malformed)?;
                            let index = match value {
                                IntegerValue::Unsigned(value) => u64::try_from(value),
                                IntegerValue::Signed(value) => u64::try_from(value),
                            }
                            .map_err(|_| malformed())?;
                            CanonicalStructuralPathSegment::FixedIndex(index)
                        }
                        ScalarReadProjection::Extent => {
                            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                                "boundary input descriptor extent has no closed evaluation",
                            ));
                        }
                    });
                }
                Some(self.read(*root, &selected, *scalar_type)?)
            }
            ScalarTerm::BooleanField { root, path } => {
                Some(self.read(*root, path, ScalarType::Boolean)?)
            }
            ScalarTerm::IntegerField {
                root,
                path,
                scalar_type,
            } => Some(self.read(*root, path, ScalarType::Integer(*scalar_type))?),
            ScalarTerm::IeeeFloatField { root, path, format } => {
                Some(self.read(*root, path, ScalarType::IeeeFloat(*format))?)
            }
            _ => None,
        };
        if let Some(value) = observed {
            *term = scalar_literal(value);
        }
        Ok(())
    }

    fn read(
        &self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<TerminalScalarValue, TerminalInterpretError> {
        let position = self
            .boundary
            .structural_parameters
            .iter()
            .position(|parameter| parameter.place == root)
            .ok_or_else(malformed)?;
        let parameter = &self.boundary.structural_parameters[position];
        if !matches!(
            parameter.access,
            terminal_psi::StructuralAccess::Owned
                | terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
        ) {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "boundary input predicate cannot observe a write-only argument",
            ));
        }
        let argument = self.arguments.get(position).ok_or_else(malformed)?;
        let value = self.values.get(position).ok_or_else(malformed)?;
        let path = terminal_semantics::canonical_structural_storage_path(
            self.execution.program.structural_types().values(),
            parameter.structural_type,
            path,
        )
        .ok_or_else(malformed)?;
        self.execution
            .read_primitive_argument(argument, value, &path, scalar_type)
    }
}

pub(super) fn closed_guard(
    proposition: &Proposition,
    integers: &mut ClosedIntegerEvaluator,
) -> Result<Option<bool>, ClosedIntegerEvaluationError> {
    Ok(match proposition {
        Proposition::Truth => Some(true),
        Proposition::Falsehood => Some(false),
        Proposition::ScalarIeeeFloatComparison {
            kind, left, right, ..
        } => {
            let (ScalarTerm::IeeeFloatLiteral(left), ScalarTerm::IeeeFloatLiteral(right)) =
                (left, right)
            else {
                return Ok(None);
            };
            use semantic_vocabulary::{
                IeeeFloatComparisonKind as Kind, IeeeFloatComparisonOperation as Operation,
            };
            let (operation, negate) = match kind {
                Kind::Equal => (Operation::Equal, false),
                Kind::NotEqual => (Operation::NotEqual, false),
                Kind::LessThan => (Operation::Less, false),
                Kind::LessOrEqual => (Operation::LessOrEqual, false),
                Kind::NotLessThan => (Operation::Less, true),
                Kind::NotLessOrEqual => (Operation::LessOrEqual, true),
            };
            Some(crate::scalar_operations::ieee_float_compare(operation, *left, *right) != negate)
        }
        Proposition::Equal(left, right) if left.scalar_type() == ScalarType::Boolean => left
            .boolean_value()
            .zip(right.boolean_value())
            .map(|(left, right)| left == right),
        semantic_vocabulary::ordered_comparison_operands!(left, right) => {
            let Some(((left_type, left), (right_type, right))) =
                left.integer_value().zip(right.integer_value())
            else {
                return Ok(None);
            };
            if left_type != right_type {
                return Ok(None);
            }
            left_type
                .compare(left, right)
                .map(|ordering| match proposition {
                    Proposition::Equal(..) => ordering.is_eq(),
                    Proposition::LessThan(..) => ordering.is_lt(),
                    _ => !ordering.is_gt(),
                })
        }
        semantic_vocabulary::integer_math_comparison_operands!(left, right) => integers
            .compare(left, right)?
            .map(|ordering| match proposition {
                Proposition::IntegerMathEqual(..) => ordering.is_eq(),
                Proposition::IntegerMathLessThan(..) => ordering.is_lt(),
                _ => !ordering.is_gt(),
            }),
        Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
            let conjunction = matches!(proposition, Proposition::Conjunction(_));
            let mut value = conjunction;
            for child in children {
                let Some(child) = closed_guard(child, integers)? else {
                    return Ok(None);
                };
                value = if conjunction {
                    value && child
                } else {
                    value || child
                };
            }
            Some(value)
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => match closed_guard(premise, integers)? {
            Some(false) => Some(true),
            Some(true) => closed_guard(conclusion, integers)?,
            None => None,
        },
        _ => None,
    })
}

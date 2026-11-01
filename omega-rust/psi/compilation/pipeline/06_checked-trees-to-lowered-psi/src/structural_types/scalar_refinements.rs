//! Field predicates bind only their one formal subject. Logical conversion is
//! shared with machine contracts; this owner supplies the smaller namespace.

use checked_trees::{CheckedBooleanExpression, CheckedScalarExpression};
use semantic_vocabulary::{ScalarRefinement, ScalarTerm, ScalarType};
use typed_trees::types::PrimitiveType;

use crate::emission::scalar_types::{integer_value, terminal_scalar_type};
use crate::lowering_error::{LoweringError, unsupported};
use crate::proofs::contract_predicates::{self, PredicateTerms};

pub(super) fn lower(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    primitive: PrimitiveType,
    predicate: &CheckedBooleanExpression,
) -> Result<ScalarRefinement, LoweringError> {
    let carrier = terminal_scalar_type(primitive)?;
    let predicate =
        contract_predicates::proposition(identities, predicate, &FieldSubject { carrier })?;
    ScalarRefinement::new(carrier, predicate).map_err(|_| {
        LoweringError::Unsupported("scalar field predicate is not closed over its carrier")
    })
}

struct FieldSubject {
    carrier: ScalarType,
}

impl PredicateTerms for FieldSubject {
    fn scalar(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: &CheckedScalarExpression,
    ) -> Result<ScalarTerm, LoweringError> {
        // Arithmetic shares the same carrier/policy conversion as callable
        // predicates. Only the leaves have a smaller, one-subject namespace.
        crate::proofs::integer_predicates::lower(
            expression,
            &[],
            &mut |expression| match expression {
                CheckedScalarExpression::Parameter {
                    position: 0,
                    primitive_type,
                } if terminal_scalar_type(*primitive_type)? == self.carrier => {
                    Ok(ScalarTerm::value(ScalarRefinement::SUBJECT, self.carrier))
                }
                CheckedScalarExpression::IntegerLiteral { literal } => {
                    let Some(integer) = self.carrier.as_integer() else {
                        return unsupported(
                            "scalar field predicate has a noninteger literal carrier",
                        );
                    };
                    ScalarTerm::integer(integer, integer_value(literal, self.carrier)?)
                        .map_err(LoweringError::InvalidCrashPredicate)
                }
                // An IEEE range endpoint of the field's own format.
                CheckedScalarExpression::IeeeFloatLiteral { value }
                    if ScalarType::IeeeFloat(value.format()) == self.carrier =>
                {
                    Ok(ScalarTerm::IeeeFloatLiteral(*value))
                }
                CheckedScalarExpression::Boolean(predicate) => self.boolean(identities, predicate),
                _ => unsupported("scalar field predicate names a value outside its formal subject"),
            },
        )
    }

    fn boolean(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: &CheckedBooleanExpression,
    ) -> Result<ScalarTerm, LoweringError> {
        match expression {
            CheckedBooleanExpression::Constant(value) => Ok(ScalarTerm::boolean(*value)),
            CheckedBooleanExpression::Parameter { position: 0 }
                if self.carrier == ScalarType::Boolean =>
            {
                Ok(ScalarTerm::value(ScalarRefinement::SUBJECT, self.carrier))
            }
            CheckedBooleanExpression::Not(operand) => {
                ScalarTerm::boolean_not(self.boolean(identities, operand)?)
                    .map_err(LoweringError::InvalidCrashPredicate)
            }
            CheckedBooleanExpression::Equal { left, right } => ScalarTerm::boolean_equal(
                self.boolean(identities, left)?,
                self.boolean(identities, right)?,
            )
            .map_err(LoweringError::InvalidCrashPredicate),
            _ => unsupported("scalar field Boolean predicate names an unsupported subject"),
        }
    }
}

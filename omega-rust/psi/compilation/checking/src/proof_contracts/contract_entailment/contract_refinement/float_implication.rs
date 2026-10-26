//! IEEE strengthening of one declaration observation. The ordinary contract
//! projector supplies positional subject identity; selected comparison meaning
//! and the actual carrier supply authority and format. Only positive predicates
//! enter FloatPredicate, so neither `!=` nor a false order can exclude NaN.
//! This is declaration substitution, not a value inferred from a provider body.

use super::{Conjunct, CorrespondenceFact, correspondence_fact};
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::types::PrimitiveType;

pub(super) struct FloatObservation {
    subject: CorrespondenceFact,
    predicate: facts::FloatPredicate,
}

impl FloatObservation {
    pub(super) fn collect(
        program: &dyn crate::CheckingInput,
        owner: SymbolHandle,
        parameters: &[StateParameter],
        binders: &[(SymbolHandle, String)],
        fact: &ProofFact,
    ) -> Vec<Self> {
        if let ProofFact::Expression(expression) = fact
            && let Some(predicates) =
                crate::mathematical_applications::float_predicates(program, *expression)
        {
            // Every conjunct must retain an exact subject and format. Dropping
            // an unread conjunct would turn a partial projection into a proof
            // of the entire public guarantee.
            return predicates
                .into_iter()
                .map(|projection| {
                    let subject = projection.subject;
                    let predicate = projection.predicate;
                    let reference = if let Some((result_owner, reference)) =
                        crate::reserved_result_owner(program, subject)
                    {
                        (result_owner == owner).then_some(reference)?
                    } else {
                        crate::parameter_expression_result_type_reference(
                            program, owner, parameters, subject,
                        )?
                    };
                    let matches = matches!(
                        (
                            program.primitive_type_reference(reference)?,
                            predicate.format()
                        ),
                        (PrimitiveType::F32, numerics::literals::FloatFormat::F32)
                            | (PrimitiveType::F64, numerics::literals::FloatFormat::F64)
                    );
                    if !matches {
                        return None;
                    }
                    Some(Self {
                        subject: correspondence_fact(
                            program,
                            &ProofFact::Expression(subject),
                            binders,
                        )?,
                        predicate,
                    })
                })
                .collect::<Option<Vec<_>>>()
                .unwrap_or_default();
        }
        Self::collect_direct(program, owner, parameters, binders, fact)
            .into_iter()
            .collect()
    }

    fn collect_direct(
        program: &dyn crate::CheckingInput,
        owner: SymbolHandle,
        parameters: &[StateParameter],
        binders: &[(SymbolHandle, String)],
        fact: &ProofFact,
    ) -> Option<Self> {
        let ProofFact::Expression(expression) = fact else {
            return None;
        };
        let binary = program.expression_table.binary_node(*expression)?;
        let literal = |value| match program.expression_table.peeled_expression(value) {
            ExpressionNode::Float(literal) => Some(literal),
            _ => None,
        };
        let (subject, endpoint, operator) = if let Some(endpoint) = literal(binary.right) {
            (binary.left, endpoint, binary.operator)
        } else {
            (
                binary.right,
                literal(binary.left)?,
                binary.operator.mirrored(),
            )
        };
        let result_type = |value: ExpressionHandle| {
            if let Some((result_owner, reference)) = crate::reserved_result_owner(program, value) {
                return (result_owner == owner).then_some(reference);
            }
            crate::parameter_expression_result_type_reference(program, owner, parameters, value)
        };
        let reference = result_type(subject)?;
        let format = match program.primitive_type_reference(reference)? {
            PrimitiveType::F32 => numerics::literals::FloatFormat::F32,
            PrimitiveType::F64 => numerics::literals::FloatFormat::F64,
            _ => return None,
        };
        if endpoint.landing().is_some_and(|landing| landing != format) {
            return None;
        }
        if !crate::has_builtin_or_sealed_float_comparison_meaning(
            program,
            owner,
            *expression,
            binary.operator,
            &[result_type(binary.left), result_type(binary.right)],
        ) {
            return None;
        }
        // Land the decimal directly in its authenticated format, avoiding a
        // double-rounded f64 intermediate for f32 contract endpoints.
        let bound = match format {
            numerics::literals::FloatFormat::F32 => {
                numerics::bignum::ExactFloat::from_f32(endpoint.value_f32())
            }
            numerics::literals::FloatFormat::F64 => {
                numerics::bignum::ExactFloat::from_f64(endpoint.value_f64())
            }
        };
        Some(Self {
            subject: correspondence_fact(program, &ProofFact::Expression(subject), binders)?,
            predicate: facts::FloatPredicate::comparison(format, operator, &bound)?,
        })
    }

    pub(super) fn entailed_by(&self, premises: &[Conjunct]) -> bool {
        premises
            .iter()
            .flat_map(|premise| premise.float.iter())
            .any(|premise| {
                premise.subject == self.subject && premise.predicate.implies(self.predicate)
            })
    }
}

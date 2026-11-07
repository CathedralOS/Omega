//! Skip a predicate row only when the case tag and enforced field restrictions
//! already guarantee it.
//!
//! An enclosing predicate can be temporarily unproved during mutation. It
//! cannot supply its own premise here; only restrictions actually retained on
//! each field's storage may make the whole declaration row redundant.

use super::{FactSpelling, carrier_bounds};
use crate::proof_contracts::arithmetic_domains::BigIntInterval;
use checked_trees::{
    CheckedBooleanExpression as Boolean, CheckedIntegerComparisonKind as Comparison,
    CheckedMemberIdentity, CheckedScalarExpression as Scalar, CheckedStructuralCaseRoot,
    CheckedStructuralPredicatePathSegment as Segment,
};
use numerics::bignum::BigInt;
use typed_trees::data::{DataDefinition, DataVariant};

pub(super) fn imply_declaration(
    program: &dyn crate::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    definition: &DataDefinition,
) -> bool {
    let spelling = FactSpelling {
        program,
        operators,
        parameter_position: 0,
        prefix: &[],
        definition,
        selected_case: None,
    };
    let Some(predicates) = program
        .proof_facts
        .span_or_empty(definition.where_facts)
        .iter()
        .map(|fact| spelling.boolean(*fact.as_expression()?))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let mut cases = program
        .data_members(definition)
        .iter()
        .filter_map(|member| member.as_variant())
        .peekable();
    if cases.peek().is_none() {
        let carriers = FieldCarriers {
            program,
            definition,
            case: None,
        };
        return predicates
            .iter()
            .all(|predicate| carriers.boolean(predicate) == Some(true));
    }
    cases.all(|case| {
        let carriers = FieldCarriers {
            program,
            definition,
            case: Some(case),
        };
        predicates
            .iter()
            .all(|predicate| carriers.boolean(predicate) == Some(true))
    })
}

struct FieldCarriers<'a> {
    program: &'a dyn crate::CheckingInput,
    definition: &'a DataDefinition,
    case: Option<&'a DataVariant>,
}

impl FieldCarriers<'_> {
    fn boolean(&self, predicate: &Boolean) -> Option<bool> {
        match predicate {
            Boolean::Constant(value) => Some(*value),
            Boolean::StructuralCaseMembership { subject, case }
                if subject.root == (CheckedStructuralCaseRoot::Parameter { position: 0 })
                    && subject.path.is_empty() =>
            {
                let selected = self.case?;
                Some(
                    *case
                        == CheckedMemberIdentity::of_member(
                            selected.identity,
                            selected.name.clone(),
                        ),
                )
            }
            Boolean::Not(operand) => Some(!self.boolean(operand)?),
            // Preserve short-circuit formation: an unknown left operand cannot
            // be bypassed because the right happens to have a constant result.
            Boolean::And { left, right } => Some(self.boolean(left)? && self.boolean(right)?),
            Boolean::Or { left, right } => Some(self.boolean(left)? || self.boolean(right)?),
            Boolean::Equal { left, right } => Some(self.boolean(left)? == self.boolean(right)?),
            Boolean::IntegerComparison { kind, left, right } => {
                let left = self.integer(left)?;
                let right = self.integer(right)?;
                let (left_low, left_high) = (&left.minimum, &left.maximum);
                let (right_low, right_high) = (&right.minimum, &right.maximum);
                match kind {
                    Comparison::LessOrEqual if left_high <= right_low => Some(true),
                    Comparison::LessOrEqual if left_low > right_high => Some(false),
                    Comparison::LessThan if left_high < right_low => Some(true),
                    Comparison::LessThan if left_low >= right_high => Some(false),
                    Comparison::Equal
                        if left_low == left_high
                            && right_low == right_high
                            && left_low == right_low =>
                    {
                        Some(true)
                    }
                    Comparison::Equal if left_high < right_low || right_high < left_low => {
                        Some(false)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn integer(&self, expression: &Scalar) -> Option<facts::IntegerRange> {
        match expression {
            Scalar::IntegerLiteral { literal } => {
                let value = literal
                    .value_i64()
                    .map(i128::from)
                    .or_else(|| literal.value_u64().map(i128::from))?;
                BigIntInterval::constant(BigInt::from_i128(value)).into_closed()
            }
            Scalar::StructuralParameterField {
                parameter_position: 0,
                path,
                primitive_type,
                ..
            } => {
                let field = match path.as_slice() {
                    [Segment::Field(identity)] => self
                        .program
                        .data_members(self.definition)
                        .iter()
                        .find_map(|member| {
                            member.as_field().filter(|field| {
                                field.identity == identity.ordinal && field.name == identity.name
                            })
                        })?,
                    [Segment::Case(case), Segment::Field(identity)] => {
                        let selected = self.case?;
                        if *case
                            != CheckedMemberIdentity::of_member(
                                selected.identity,
                                selected.name.clone(),
                            )
                        {
                            return None;
                        }
                        self.program
                            .data_payload_fields(selected)
                            .iter()
                            .find(|field| {
                                field.identity == identity.ordinal && field.name == identity.name
                            })?
                    }
                    _ => return None,
                };
                if field.relevance.is_erased()
                    || self.program.primitive_type_reference(field.type_reference)
                        != Some(*primitive_type)
                {
                    return None;
                }
                let (low, high) = carrier_bounds(*primitive_type)?;
                let mut interval = BigIntInterval {
                    low: Some(BigInt::from_i128(low)),
                    high: Some(BigInt::from_i128(high)),
                };
                if let Some((low, high)) =
                    crate::enforced_integer_type_bounds(self.program, field.type_reference)
                {
                    interval = interval.intersect(&BigIntInterval {
                        low: Some(BigInt::from_i64(low)),
                        high: Some(BigInt::from_i64(high)),
                    });
                }
                // Dedicated atomic fields keep resident bounds at every event;
                // ordinary data predicates never narrow a plain scalar slot.
                if let Some(resident) =
                    crate::proof_contracts::default_domains::atomic_field_interval(
                        self.program,
                        self.definition,
                        field,
                    )
                {
                    interval = interval.intersect(&BigIntInterval {
                        low: resident.low.map(BigInt::from_i64),
                        high: resident.high.map(BigInt::from_i64),
                    });
                }
                interval.into_closed()
            }
            _ => None,
        }
    }
}

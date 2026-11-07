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

#[cfg(test)]
mod tests {
    use super::*;
    use arena::HandleSpan;
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::{TypedTrees, data::DataField, name::Identifier, types::TypeReferenceNode};

    #[test]
    fn payload_carriers_prove_only_the_complete_guarded_bound() {
        let mut symbols = SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = symbols.insert_children(
            root,
            symbols::builtin_type_symbols()
                .into_iter()
                .chain([(SymbolKind::Field, SymbolNameRef::Static("count"))]),
        );
        let integer = children.handle_at(symbols::BuiltinTypeAtom::U64.ordinal() as u32);
        let field_symbol = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let integer_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: integer,
                name: Identifier::from("u64"),
            });
        let minimum =
            program
                .expression_table
                .insert(typed_trees::expression::ExpressionNode::Integer(
                    numerics::literals::IntegerLiteral::from_value(0),
                ));
        let maximum =
            program
                .expression_table
                .insert(typed_trees::expression::ExpressionNode::Integer(
                    numerics::literals::IntegerLiteral::from_value(256),
                ));
        let constraints = program.type_reference_table.insert_constraints([
            typed_trees::types::TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive: true,
            },
        ]);
        let bounded_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: integer_type,
                constraints,
            });
        let mut payload = HandleSpan::empty();
        program.data_payload_fields.append_to_span(
            &mut payload,
            DataField {
                identity: Some(3),
                symbol: field_symbol,
                name: Identifier::from("count"),
                type_reference: bounded_type,
                ..Default::default()
            },
        );
        let case = DataVariant {
            identity: Some(2),
            name: Identifier::from("Full"),
            payload,
            ..Default::default()
        };
        let identity = CheckedMemberIdentity::of_member(case.identity, case.name.clone());
        let definition = DataDefinition::default();
        let mut raw_payload = HandleSpan::empty();
        program.data_payload_fields.append_to_span(
            &mut raw_payload,
            DataField {
                identity: Some(3),
                symbol: field_symbol,
                name: Identifier::from("count"),
                type_reference: integer_type,
                ..Default::default()
            },
        );
        let unrestricted_case = DataVariant {
            payload: raw_payload,
            ..case.clone()
        };
        let empty_case = DataVariant {
            identity: Some(1),
            name: Identifier::from("Empty"),
            ..Default::default()
        };
        let carriers = FieldCarriers {
            program: &program,
            definition: &definition,
            case: Some(&case),
        };
        let membership = |case| Boolean::StructuralCaseMembership {
            subject: checked_trees::CheckedStructuralCaseSubject {
                root: CheckedStructuralCaseRoot::Parameter { position: 0 },
                path: Vec::new(),
            },
            case,
        };
        let leaf = Scalar::StructuralParameterField {
            observation: Default::default(),
            parameter_position: 0,
            path: vec![
                Segment::Case(identity.clone()),
                Segment::Field(CheckedMemberIdentity::of_member(Some(3), "count".into())),
            ],
            primitive_type: typed_trees::types::PrimitiveType::U64,
        };
        let bound = |maximum| Boolean::IntegerComparison {
            kind: Comparison::LessOrEqual,
            left: Box::new(leaf.clone()),
            right: Box::new(Scalar::IntegerLiteral {
                literal: numerics::literals::IntegerLiteral::from_value(maximum),
            }),
        };
        let guarded = |maximum| Boolean::Or {
            left: Box::new(membership(CheckedMemberIdentity::of_member(
                Some(1),
                "Empty".into(),
            ))),
            right: Box::new(Boolean::And {
                left: Box::new(membership(identity.clone())),
                right: Box::new(bound(maximum)),
            }),
        };
        assert_eq!(carriers.boolean(&guarded(256)), Some(true));
        assert_eq!(
            carriers.boolean(&guarded(255)),
            None,
            "a stronger predicate must retain its row"
        );
        let unrestricted = FieldCarriers {
            program: &program,
            definition: &definition,
            case: Some(&unrestricted_case),
        };
        assert_eq!(
            unrestricted.boolean(&guarded(256)),
            None,
            "the source predicate cannot establish its own field bound"
        );
        let empty = FieldCarriers {
            program: &program,
            definition: &definition,
            case: Some(&empty_case),
        };
        assert_eq!(
            empty.boolean(&guarded(256)),
            Some(true),
            "inactive payloads are not read"
        );
        assert_eq!(
            empty.boolean(&bound(256)),
            None,
            "an unguarded inactive payload is not a premise"
        );
        assert_eq!(
            empty.boolean(&Boolean::Or {
                left: Box::new(bound(256)),
                right: Box::new(Boolean::Constant(true))
            }),
            None
        );
    }
}

//! Discharge declaration rows already enforced by selected payload carriers.
//!
//! A sum's tag selects exactly one payload. Each payload's interval-only
//! contract is retained on its scalar fields, independently checked at stores
//! and assumed at reads. Prove the *whole* declaration predicate from those
//! carriers for every case before omitting its redundant expanded roster.
//! Unknown predicates stay in the roster; a useful consequence is insufficient.

use super::{FactSpelling, carrier_bounds};
use checked_trees::{
    CheckedBooleanExpression as Boolean, CheckedIntegerComparisonKind as Comparison,
    CheckedMemberIdentity, CheckedOperatorFacts, CheckedScalarExpression as Scalar,
    CheckedStructuralCaseRoot, CheckedStructuralPredicatePathSegment as Segment,
};
use typed_trees::data::{DataDefinition, DataVariant};

pub(super) fn imply_declaration(
    program: &dyn crate::CheckingInput,
    operators: &CheckedOperatorFacts,
    definition: &DataDefinition,
) -> bool {
    let mut cases = program
        .data_members(definition)
        .iter()
        .filter_map(|member| member.as_variant())
        .peekable();
    if cases.peek().is_none() {
        return false;
    }
    let spelling = FactSpelling {
        program,
        operators,
        parameter_position: 0,
        prefix: &[],
        definition,
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
    cases.all(|case| {
        let Some(intervals) = crate::case_where_field_intervals(program, case) else {
            return false;
        };
        let carriers = CaseCarriers {
            program,
            case,
            identity: CheckedMemberIdentity::of_member(case.identity, case.name.clone()),
            intervals: &intervals,
        };
        predicates
            .iter()
            .all(|predicate| carriers.boolean(predicate) == Some(true))
    })
}

struct CaseCarriers<'a> {
    program: &'a dyn crate::CheckingInput,
    case: &'a DataVariant,
    identity: CheckedMemberIdentity,
    intervals: &'a [(symbols::SymbolHandle, Option<i64>, Option<i64>)],
}

impl CaseCarriers<'_> {
    fn boolean(&self, predicate: &Boolean) -> Option<bool> {
        match predicate {
            Boolean::Constant(value) => Some(*value),
            Boolean::StructuralCaseMembership { subject, case }
                if subject.root == (CheckedStructuralCaseRoot::Parameter { position: 0 })
                    && subject.path.is_empty() =>
            {
                Some(*case == self.identity)
            }
            Boolean::Not(operand) => Some(!self.boolean(operand)?),
            // Preserve short-circuit formation: an unknown left operand cannot
            // be bypassed because the right happens to have a constant result.
            Boolean::And { left, right } => Some(self.boolean(left)? && self.boolean(right)?),
            Boolean::Or { left, right } => Some(self.boolean(left)? || self.boolean(right)?),
            Boolean::Equal { left, right } => Some(self.boolean(left)? == self.boolean(right)?),
            Boolean::IntegerComparison { kind, left, right } => {
                let (left_low, left_high) = self.integer(left)?;
                let (right_low, right_high) = self.integer(right)?;
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

    fn integer(&self, expression: &Scalar) -> Option<(i128, i128)> {
        match expression {
            Scalar::IntegerLiteral { literal } => {
                let value = literal
                    .value_i64()
                    .map(i128::from)
                    .or_else(|| literal.value_u64().map(i128::from))?;
                Some((value, value))
            }
            Scalar::StructuralParameterField {
                parameter_position: 0,
                path,
                primitive_type,
                ..
            } => {
                let [Segment::Case(case), Segment::Field(identity)] = path.as_slice() else {
                    return None;
                };
                if *case != self.identity {
                    return None;
                }
                let field = self
                    .program
                    .data_payload_fields(self.case)
                    .iter()
                    .find(|field| {
                        field.identity == identity.ordinal && field.name == identity.name
                    })?;
                if field.relevance.is_erased()
                    || self.program.primitive_type_reference(field.type_reference)
                        != Some(*primitive_type)
                {
                    return None;
                }
                let (mut low, mut high) = carrier_bounds(*primitive_type)?;
                if let Some((_, lower, upper)) = self
                    .intervals
                    .iter()
                    .find(|(symbol, _, _)| *symbol == field.symbol)
                {
                    if let Some(lower) = lower {
                        low = low.max(i128::from(*lower));
                    }
                    if let Some(upper) = upper {
                        high = high.min(i128::from(*upper));
                    }
                }
                (low <= high).then_some((low, high))
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
        let integer = symbols.insert_root(SymbolKind::BuiltinType, SymbolNameRef::Static("u64"));
        let field_symbol = symbols.insert_root(SymbolKind::Field, SymbolNameRef::Static("count"));
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
        let mut payload = HandleSpan::empty();
        program.data_payload_fields.append_to_span(
            &mut payload,
            DataField {
                identity: Some(3),
                symbol: field_symbol,
                name: Identifier::from("count"),
                type_reference: integer_type,
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
        let intervals = [(field_symbol, None, Some(256))];
        let carriers = CaseCarriers {
            program: &program,
            case: &case,
            identity: identity.clone(),
            intervals: &intervals,
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
        let unrestricted = CaseCarriers {
            intervals: &[],
            ..carriers
        };
        assert_eq!(
            unrestricted.boolean(&guarded(256)),
            None,
            "the source predicate cannot establish its own field bound"
        );
        let empty = CaseCarriers {
            identity: CheckedMemberIdentity::of_member(Some(1), "Empty".into()),
            ..unrestricted
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

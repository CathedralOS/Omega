//! Exact normalized theorem roles and result relation substitutions.
use super::*;
pub(super) fn validate_congruence_schema(
    contract: &language_semantics::quotient_correspondence::CanonicalQuotientCorrespondence,
) -> Result<(), &'static str> {
    use language_semantics::quotient_correspondence::{
        QuotientPositionalRelation as Relation, QuotientResultRelation as ResultRelation,
        QuotientTheoremCorrespondence as Schema, QuotientTheoremParameterRole as Role,
    };
    let Some(Schema::Congruence(schema)) = contract
        .theorem_evidence
        .first()
        .map(|row| &row.correspondence)
    else {
        return Err("quotient congruence schema is absent");
    };
    let mut roles = Vec::new();
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut premises = Vec::new();
    for (position, relation) in contract.input_relations.iter().enumerate() {
        let input_position =
            u32::try_from(position).map_err(|_| "quotient schema position overflow")?;
        let first = u32::try_from(roles.len()).map_err(|_| "quotient schema parameter overflow")?;
        match relation {
            Relation::Quotient(relation) => {
                roles.push(Role::QuotientLeft { input_position });
                roles.push(Role::QuotientRight { input_position });
                left.push(first);
                right.push(first + 1);
                premises.push((&relation.relation, first, first + 1));
            }
            Relation::ExactEquality { .. } => {
                roles.push(Role::Shared { input_position });
                left.push(first);
                right.push(first);
            }
        }
    }
    if schema.parameters.len() != roles.len()
        || schema
            .parameters
            .iter()
            .zip(roles)
            .enumerate()
            .any(|(position, (actual, role))| {
                actual.theorem_position as usize != position || actual.role != role
            })
        || schema.relation_premises.len() != premises.len()
        || schema
            .relation_premises
            .iter()
            .zip(premises)
            .enumerate()
            .any(|(position, (actual, (relation, left, right)))| {
                actual.expected_position as usize != position
                    || &actual.relation != relation
                    || actual.left_parameter != left
                    || actual.right_parameter != right
            })
        || schema.conclusion.left.arguments != left
        || schema.conclusion.right.arguments != right
    {
        return Err("quotient normalized congruence schema drifted");
    }
    let expected_result = match &contract.result_relation {
        Relation::Quotient(relation) => ResultRelation::Quotient(relation.relation.clone()),
        Relation::ExactEquality {
            representative_type,
            ..
        } => ResultRelation::ExactEquality(representative_type.clone()),
    };
    if schema.conclusion.relation != expected_result {
        return Err("quotient congruence result relation drifted");
    }
    let mut coordinates = Vec::new();
    for premise in &schema.relation_premises {
        if coordinates.contains(&premise.actual) {
            return Err("quotient theorem reuses a premise coordinate");
        }
        coordinates.push(premise.actual);
    }
    if coordinates.contains(&schema.conclusion.actual) {
        return Err("quotient theorem conclusion reuses a premise coordinate");
    }
    Ok(())
}

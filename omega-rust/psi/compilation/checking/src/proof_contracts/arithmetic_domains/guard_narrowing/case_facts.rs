//! Selected-arm case-fact contribution (CASE-CONSTRAINTS, ch12).
//!
//! A positive `subject in Type::Case` guard leaf -- the arm marker a
//! `Type::Case { .. }` / `Type::Case` transition pattern desugars to, or an
//! authored membership -- establishes the matched case's `where` facts on the
//! subject for everything that arm evaluates: dispatch proved the case, so its
//! facts are premises at that program point. Facts are projected onto the
//! subject's place (`lo` inside `where lo <= hi` reads `subject.lo`) and
//! contributed by `projected_facts` through the same environment carriers an
//! authored comparison would produce: ordered relations, place intervals,
//! `known_u64` values, and joint-subtract bounds. A negated membership contributes nothing (exclusion
//! is coverage's business); a fact shape these carriers cannot hold
//! contributes nothing either -- a missed premise is sound, a wrong one is
//! not. Ordinary write invalidation retires the projected paths exactly like
//! guard-derived facts, so a payload write inside the arm cannot leave a
//! stale `where` fact believed.
use super::projected_facts::{
    ProjectedOperand, contribute_projected_comparison, has_fixed_integer_meaning,
};
use super::{
    BinaryOperator, ExpressionHandle, ExpressionNode, Machine, State, ValueEnvironment, meaning,
    ordered_values,
};
use typed_trees::data::{DataDefinition, DataMember, DataVariant};

/// Establish `classifier`'s case `where` facts on `subject` inside `environment`.
/// `classifier` must rejoin its complete retained case reference (the same
/// ownership spine `has_exact_case_membership_meaning` requires); the subject
/// must be an operand-identifiable place, since the contribution is defined
/// as a projection onto that place.
pub(super) fn contribute_case_where_facts(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &mut ValueEnvironment,
    subject: ExpressionHandle,
    classifier: ExpressionHandle,
) {
    let Some(state) = state else {
        return;
    };
    let Some(owner) = meaning::exact_case_reference_owner(program, classifier) else {
        return;
    };
    let Some(case) = program.expression_table.name_path_node(classifier) else {
        return;
    };
    let Some(variant) = program.data_variant(owner, case.symbol) else {
        return;
    };
    if variant.where_facts.is_empty() {
        return;
    }
    let Some(subject) = ordered_values::operand(program, machine, state, subject, None) else {
        return;
    };
    for fact in program.proof_facts.span_or_empty(variant.where_facts) {
        let Some(expression) = fact.as_expression() else {
            continue;
        };
        contribute_fact_expression(program, owner, variant, &subject, environment, *expression);
    }
}

/// One case `where` fact: `a && b` distributes (each conjunct holds on the
/// arm); every other shape is a single comparison leaf or contributes
/// nothing through these carriers.
fn contribute_fact_expression(
    program: &dyn crate::CheckingInput,
    owner: &DataDefinition,
    variant: &DataVariant,
    subject: &ordered_values::Operand,
    environment: &mut ValueEnvironment,
    expression: ExpressionHandle,
) {
    let Some(comparison) = program.expression_table.binary_node(expression) else {
        return;
    };
    if comparison.operator == BinaryOperator::And {
        contribute_fact_expression(
            program,
            owner,
            variant,
            subject,
            environment,
            comparison.left,
        );
        contribute_fact_expression(
            program,
            owner,
            variant,
            subject,
            environment,
            comparison.right,
        );
        return;
    }
    let (Some(left), Some(right)) = (
        fact_side(program, owner, variant, subject, comparison.left),
        fact_side(program, owner, variant, subject, comparison.right),
    ) else {
        return;
    };
    contribute_projected_comparison(
        program,
        environment,
        comparison.operator,
        (&left, comparison.left),
        (&right, comparison.right),
    );
}

/// Project one side of a case `where` fact onto the dispatched subject.
/// A `Name` leaf resolves to the case's payload or the record's common field
/// (symbol first, the fact's own field-name spelling when the leaf symbol was
/// not retained); a literal stays a literal. Anything else -- calls, member
/// chains, arithmetic inside the fact -- has no operand projection here and
/// skips the whole comparison.
fn fact_side(
    program: &dyn crate::CheckingInput,
    owner: &DataDefinition,
    variant: &DataVariant,
    subject: &ordered_values::Operand,
    expression: ExpressionHandle,
) -> Option<ProjectedOperand> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal)
            if literal.landing().is_none_or(|landing| {
                landing.landed_type != numerics::literals::LandedIntegerType::Addr
            }) =>
        {
            Some(ProjectedOperand::literal(literal))
        }
        ExpressionNode::Name(path) => {
            let name = program
                .expression_table
                .name_path_members(path.members)
                .last()?
                .as_str();
            let field = if path.symbol.is_valid() {
                payload_field(program, variant)
                    .chain(common_fields(program, owner))
                    .find(|field| field.symbol == path.symbol)?
            } else {
                let mut matches = payload_field(program, variant)
                    .chain(common_fields(program, owner))
                    .filter(|field| field.name.as_str() == name);
                let field = matches.next()?;
                if matches.next().is_some() {
                    return None;
                }
                field
            };
            // This environment records integer differences (a strict order
            // contributes a distance of at least one). A case's IEEE or
            // nominal facts cannot supply those relations or integer intervals.
            if !has_fixed_integer_meaning(program, field.type_reference) {
                return None;
            }
            let operand = ordered_values::Operand::Field {
                receiver: Box::new(subject.clone()),
                field: field.symbol,
                name: field.name.as_str().to_owned(),
                primitive: program.primitive_type_reference(field.type_reference),
            };
            let spelled = field_name_unique(program, owner, field.name.as_str())
                .then(|| operand.static_path())
                .flatten();
            Some(ProjectedOperand {
                operand: Some(operand),
                spelled,
                declared_type: Some(field.type_reference),
            })
        }
        _ => None,
    }
}

fn payload_field<'a, 'program>(
    program: &'program dyn crate::CheckingInput,
    variant: &'a DataVariant,
) -> impl Iterator<Item = &'program typed_trees::data::DataField> + 'a
where
    'program: 'a,
{
    program.data_payload_fields(variant).iter()
}

fn common_fields<'a, 'program>(
    program: &'program dyn crate::CheckingInput,
    owner: &'a DataDefinition,
) -> impl Iterator<Item = &'program typed_trees::data::DataField> + 'a
where
    'program: 'a,
{
    program.data_members(owner).iter().filter_map(|member| {
        let field = member.as_field()?;
        Some(field)
    })
}

/// `subject.<name>` is an unambiguous environment key only when exactly one field of
/// the whole data carries that name; a payload field sharing a common field's
/// spelling would otherwise let the case fact tighten the wrong reads.
fn field_name_unique(
    program: &dyn crate::CheckingInput,
    owner: &DataDefinition,
    name: &str,
) -> bool {
    let mut count = 0usize;
    for member in program.data_members(owner) {
        match member {
            DataMember::Field(field) if field.name.as_str() == name => count += 1,
            DataMember::Variant(variant) => {
                count += program
                    .data_payload_fields(variant)
                    .iter()
                    .filter(|field| field.name.as_str() == name)
                    .count();
            }
            _ => {}
        }
    }
    count == 1
}

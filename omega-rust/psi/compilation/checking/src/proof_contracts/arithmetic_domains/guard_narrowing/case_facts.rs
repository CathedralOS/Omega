//! Selected-arm case-fact contribution (CASE-CONSTRAINTS, ch12).
//!
//! A positive `subject in Type::Case` guard leaf -- the arm marker a
//! `Type::Case { .. }` / `Type::Case` transition pattern desugars to, or an
//! authored membership -- establishes the matched case's default-domain
//! requirements on the subject for everything that arm evaluates: dispatch
//! proved the case, so the requirements the data `requires` places on it are
//! premises at that program point. The declaration owner projects those
//! requirements for the selected case; this module maps each one onto the
//! subject's place (`self.lo` inside `self.lo <= self.hi` reads
//! `subject.lo`) and contributes it through `projected_facts` with the same
//! environment carriers an authored comparison would produce: ordered
//! relations, place intervals, `known_u64` values, and joint-subtract bounds.
//! A negated membership contributes nothing (exclusion is coverage's
//! business); a requirement shape these carriers cannot hold contributes
//! nothing either -- a missed premise is sound, a wrong one is not. Ordinary
//! write invalidation retires the projected paths exactly like guard-derived
//! facts, so a payload write inside the arm cannot leave a stale requirement
//! believed.
use super::projected_facts::{
    ProjectedOperand, contribute_projected_comparison, has_fixed_integer_meaning,
};
use super::{
    BinaryOperator, ExpressionHandle, ExpressionNode, Machine, State, ValueEnvironment, meaning,
    ordered_values,
};
use crate::CheckingInput;
use crate::frozen_program::expression_table;
use typed_trees::data::{DataDefinition, DataMember, DataVariant};

/// Establish the selected application's case requirements on the exact
/// subject. The authored classifier can still name a generic template; its
/// unspecialized facts are not the receiving application's facts. Reuse the
/// same authenticated membership correspondence as tag/payload checking before
/// projecting them.
pub(super) fn contribute_case_requirements(
    program: &dyn CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &mut ValueEnvironment,
    subject: ExpressionHandle,
    condition: ExpressionHandle,
    comparison: &typed_trees::expression::TableBinaryExpression,
) {
    let Some(state) = state else {
        return;
    };
    let Some(selected) = meaning::applied_case_membership_variant(
        program,
        machine,
        Some(state),
        condition,
        comparison,
    ) else {
        return;
    };
    let Some((owner_index, _)) = program.data_variant_position_for_symbol(selected) else {
        return;
    };
    let owner = &program.data_definitions()[owner_index as usize];
    let Some(variant) = program.data_variant(owner, selected) else {
        return;
    };
    let requirements = crate::proof_contracts::default_domains::selected_case_requirements(
        program, owner, selected,
    );
    if requirements.is_empty() {
        return;
    }
    let Some(subject) = ordered_values::operand(program, machine, state, subject, None) else {
        return;
    };
    for requirement in requirements {
        contribute_fact_expression(program, owner, variant, &subject, environment, requirement);
    }
}

/// One case requirement: `a && b` distributes (each conjunct holds on the
/// arm); every other shape is a single comparison leaf or contributes
/// nothing through these carriers.
fn contribute_fact_expression(
    program: &dyn CheckingInput,
    owner: &DataDefinition,
    variant: &DataVariant,
    subject: &ordered_values::Operand,
    environment: &mut ValueEnvironment,
    expression: ExpressionHandle,
) {
    let Some(comparison) = expression_table(program).binary_node(expression) else {
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

/// Project one side of a case requirement onto the dispatched subject.
/// A `self.field` leaf over the predicate subject, or a bare `Name` leaf,
/// resolves to the case's payload or the record's common field (symbol first,
/// the leaf's own field-name spelling when its symbol was not retained); a
/// literal stays a literal. Anything else -- calls, longer member chains,
/// arithmetic inside the requirement -- has no operand projection here and
/// skips the whole comparison.
fn fact_side(
    program: &dyn CheckingInput,
    owner: &DataDefinition,
    variant: &DataVariant,
    subject: &ordered_values::Operand,
    expression: ExpressionHandle,
) -> Option<ProjectedOperand> {
    match expression_table(program).expression(expression) {
        ExpressionNode::Integer(literal)
            if literal.landing().is_none_or(|landing| {
                landing.landed_type != numerics::literals::LandedIntegerType::Addr
            }) =>
        {
            Some(ProjectedOperand::literal(literal))
        }
        ExpressionNode::Name(path) => {
            let name = expression_table(program)
                .name_path_members(path.members)
                .last()?
                .as_str();
            field_operand(program, owner, variant, subject, path.symbol, name)
        }
        ExpressionNode::Member(member)
            if names_predicate_subject(program, owner, member.receiver) =>
        {
            field_operand(
                program,
                owner,
                variant,
                subject,
                member.member_symbol,
                member.member.as_str(),
            )
        }
        _ => None,
    }
}

fn names_predicate_subject(
    program: &dyn CheckingInput,
    owner: &DataDefinition,
    receiver: ExpressionHandle,
) -> bool {
    let Some(path) = expression_table(program).name_path_node(receiver) else {
        return false;
    };
    owner.predicate_subject_symbol.is_valid()
        && path.symbol == owner.predicate_subject_symbol
        && expression_table(program)
            .name_path_members(path.members)
            .len()
            == 1
}

fn field_operand(
    program: &dyn CheckingInput,
    owner: &DataDefinition,
    variant: &DataVariant,
    subject: &ordered_values::Operand,
    symbol: symbols::SymbolHandle,
    name: &str,
) -> Option<ProjectedOperand> {
    let field = if symbol.is_valid() {
        payload_field(program, variant)
            .chain(common_fields(program, owner))
            .find(|field| field.symbol == symbol)?
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

fn payload_field<'a, 'program>(
    program: &'program dyn CheckingInput,
    variant: &'a DataVariant,
) -> impl Iterator<Item = &'program typed_trees::data::DataField> + 'a
where
    'program: 'a,
{
    program.data_payload_fields(variant).iter()
}

fn common_fields<'a, 'program>(
    program: &'program dyn CheckingInput,
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
fn field_name_unique(program: &dyn CheckingInput, owner: &DataDefinition, name: &str) -> bool {
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

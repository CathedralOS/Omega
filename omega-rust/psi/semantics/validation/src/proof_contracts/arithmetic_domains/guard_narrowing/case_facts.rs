//! Selected-arm case-fact contribution (CASE-CONSTRAINTS, ch12).
//!
//! A positive `subject in Type::Case` guard leaf -- the arm marker a
//! `Type::Case { .. }` / `Type::Case` transition pattern desugars to, or an
//! authored membership -- establishes the matched case's `where` facts on the
//! subject for everything that arm evaluates: dispatch proved the case, so its
//! facts are premises at that program point. Facts are projected onto the
//! subject's place (`lo` inside `where lo <= hi` reads `subject.lo`) and
//! contributed through the same environment carriers an authored comparison would
//! produce: ordered relations, place intervals, `known_u64` values, and
//! joint-subtract bounds. A negated membership contributes nothing (exclusion
//! is coverage's business); a fact shape these carriers cannot hold
//! contributes nothing either -- a missed premise is sound, a wrong one is
//! not. Ordinary write invalidation retires the projected paths exactly like
//! guard-derived facts, so a payload write inside the arm cannot leave a
//! stale `where` fact believed.
use super::super::TypeReferenceHandle;
use super::{
    ArithmeticDomain, BinaryOperator, ExpressionHandle, ExpressionNode, Interval, Machine,
    PrimitiveType, ProofFact, State, TypedTrees, ValueEnvironment, comparison_interval,
    literal_i64, meaning, ordered_values, primitive_range, range_constraint_interval,
};
use symbols::SymbolHandle;
use typed_trees::data::{DataDefinition, DataMember, DataVariant};

/// A fact operand projected onto the dispatched subject: the ordered-values
/// identity (symbol-exact field symbols, so a payload `x` and a common `x`
/// never collide in a relation), plus the spelled path and declared field
/// type the spelling-keyed environment carriers need. `spelled` is `None` when the
/// field name is not unique across the whole data -- `subject.x` would then
/// key the same spelling onto a different field's reads, so only the
/// symbol-exact relation may be contributed.
struct FactOperand {
    operand: ordered_values::Operand,
    spelled: Option<String>,
    field_type: Option<TypeReferenceHandle>,
}

/// Establish `classifier`'s case `where` facts on `subject` inside `environment`.
/// `classifier` must rejoin its complete retained case reference (the same
/// ownership spine `has_exact_case_membership_meaning` requires); the subject
/// must be an operand-identifiable place, since the contribution is defined
/// as a projection onto that place.
pub(super) fn contribute_case_where_facts(
    program: &TypedTrees,
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
    let ExpressionNode::Name(case) = program.expression_table.expression(classifier) else {
        return;
    };
    let Some(variant) = program.data_members(owner).iter().find_map(|member| {
        let DataMember::Variant(variant) = member else {
            return None;
        };
        (variant.symbol == case.symbol).then_some(variant)
    }) else {
        return;
    };
    if variant.where_facts.is_empty() {
        return;
    }
    let Some(ordered_values::Operand::Place { root, fields, path }) =
        ordered_values::operand(program, machine, state, subject)
    else {
        return;
    };
    for fact in program.proof_facts.span_or_empty(variant.where_facts) {
        let ProofFact::Expression(expression) = fact else {
            continue;
        };
        contribute_fact_expression(
            program,
            owner,
            variant,
            &root,
            &fields,
            &path,
            environment,
            *expression,
        );
    }
}

/// One case `where` fact: `a && b` distributes (each conjunct holds on the
/// arm); every other shape is a single comparison leaf or contributes
/// nothing through these carriers.
fn contribute_fact_expression(
    program: &TypedTrees,
    owner: &DataDefinition,
    variant: &DataVariant,
    subject_root: &SymbolHandle,
    subject_fields: &[SymbolHandle],
    subject_path: &str,
    environment: &mut ValueEnvironment,
    expression: ExpressionHandle,
) {
    let ExpressionNode::Binary(comparison) = program.expression_table.expression(expression) else {
        return;
    };
    if comparison.operator == BinaryOperator::And {
        contribute_fact_expression(
            program,
            owner,
            variant,
            subject_root,
            subject_fields,
            subject_path,
            environment,
            comparison.left,
        );
        contribute_fact_expression(
            program,
            owner,
            variant,
            subject_root,
            subject_fields,
            subject_path,
            environment,
            comparison.right,
        );
        return;
    }
    let (Some(left), Some(right)) = (
        fact_side(
            program,
            owner,
            variant,
            subject_root,
            subject_fields,
            subject_path,
            comparison.left,
        ),
        fact_side(
            program,
            owner,
            variant,
            subject_root,
            subject_fields,
            subject_path,
            comparison.right,
        ),
    ) else {
        return;
    };
    match comparison.operator {
        BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual => {
            // `record`'s normalization: `high - low >= floor`, floor 1 for
            // the strict forms.
            let (high, low, strict) = match comparison.operator {
                BinaryOperator::LessOrEqual => (&right, &left, false),
                BinaryOperator::Less => (&right, &left, true),
                BinaryOperator::GreaterOrEqual => (&left, &right, false),
                _ => (&left, &right, true),
            };
            let relation = ordered_values::Relation {
                left: high.operand.clone(),
                right: low.operand.clone(),
                floor: i64::from(strict),
            };
            if !environment.ordered_values.contains(&relation) {
                environment.ordered_values.push(relation);
            }
            // The unsigned `low <= high` premise is exactly the totality
            // condition for `high - low` -- `joint_subtract_guard`'s carrier.
            if !strict
                && let (Some(high_path), Some(low_path)) =
                    (high.spelled.as_ref(), low.spelled.as_ref())
                && let (Some(high_type), Some(low_type)) = (high.field_type, low.field_type)
                && program
                    .primitive_type_reference(high_type)
                    .is_some_and(|primitive| {
                        matches!(
                            primitive,
                            PrimitiveType::U8
                                | PrimitiveType::U16
                                | PrimitiveType::U32
                                | PrimitiveType::U64
                        ) && program.primitive_type_reference(low_type) == Some(primitive)
                            && program.arithmetic_domain_for_type_reference(high_type)
                                == ArithmeticDomain::Exact
                            && program.arithmetic_domain_for_type_reference(low_type)
                                == ArithmeticDomain::Exact
                    })
            {
                environment.mark_joint_subtract_bound(high_path.clone(), low_path.clone());
            }
            // A literal side narrows the place side's interval, intersected
            // with the field's declared ranges exactly like a guard leaf.
            for (place, literal_expression, place_on_left) in [
                (&left, comparison.right, true),
                (&right, comparison.left, false),
            ] {
                let (Some(path), Some(field_type), Some(literal)) = (
                    place.spelled.as_ref(),
                    place.field_type,
                    literal_i64(program, literal_expression),
                ) else {
                    continue;
                };
                let mut interval = comparison_interval(
                    comparison.operator,
                    Interval {
                        low: Some(literal),
                        high: Some(literal),
                    },
                    place_on_left,
                );
                if let Some(type_interval) = program
                    .primitive_type_reference(field_type)
                    .and_then(primitive_range)
                {
                    interval = interval.intersect(type_interval);
                }
                if let Some(declared) = range_constraint_interval(program, field_type) {
                    interval = interval.intersect(declared);
                }
                environment.narrow(path.clone(), interval);
            }
        }
        BinaryOperator::Equal => {
            for (place, literal_expression) in
                [(&left, comparison.right), (&right, comparison.left)]
            {
                let (Some(path), Some(field_type), Some(literal)) = (
                    place.spelled.as_ref(),
                    place.field_type,
                    literal_i64(program, literal_expression),
                ) else {
                    continue;
                };
                let mut interval = Interval {
                    low: Some(literal),
                    high: Some(literal),
                };
                if let Some(type_interval) = program
                    .primitive_type_reference(field_type)
                    .and_then(primitive_range)
                {
                    interval = interval.intersect(type_interval);
                }
                if let Some(declared) = range_constraint_interval(program, field_type) {
                    interval = interval.intersect(declared);
                }
                environment.narrow(path.clone(), interval);
                if program.primitive_type_reference(field_type) == Some(PrimitiveType::U64)
                    && let Ok(value) = u64::try_from(literal)
                {
                    environment.mark_known_u64(path.clone(), value);
                }
            }
        }
        _ => {}
    }
}

/// Project one side of a case `where` fact onto the dispatched subject.
/// A `Name` leaf resolves to the case's payload or the record's common field
/// (symbol first, the fact's own field-name spelling when the leaf symbol was
/// not retained); a literal stays a literal. Anything else -- calls, member
/// chains, arithmetic inside the fact -- has no operand projection here and
/// skips the whole comparison.
fn fact_side(
    program: &TypedTrees,
    owner: &DataDefinition,
    variant: &DataVariant,
    subject_root: &SymbolHandle,
    subject_fields: &[SymbolHandle],
    subject_path: &str,
    expression: ExpressionHandle,
) -> Option<FactOperand> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => Some(FactOperand {
            operand: ordered_values::Operand::Integer(literal.clone(), literal.landing()),
            spelled: None,
            field_type: None,
        }),
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
            let mut fields = subject_fields.to_vec();
            fields.push(field.symbol);
            let path = format!("{subject_path}.{}", field.name.as_str());
            Some(FactOperand {
                operand: ordered_values::Operand::Place {
                    root: *subject_root,
                    fields,
                    path: path.clone(),
                },
                spelled: field_name_unique(program, owner, field.name.as_str()).then_some(path),
                field_type: Some(field.type_reference),
            })
        }
        _ => None,
    }
}

fn payload_field<'a, 'program>(
    program: &'program TypedTrees,
    variant: &'a DataVariant,
) -> impl Iterator<Item = &'program typed_trees::data::DataField> + 'a
where
    'program: 'a,
{
    program.data_payload_fields(variant).iter()
}

fn common_fields<'a, 'program>(
    program: &'program TypedTrees,
    owner: &'a DataDefinition,
) -> impl Iterator<Item = &'program typed_trees::data::DataField> + 'a
where
    'program: 'a,
{
    program.data_members(owner).iter().filter_map(|member| {
        let DataMember::Field(field) = member else {
            return None;
        };
        Some(field)
    })
}

/// `subject.<name>` is an unambiguous environment key only when exactly one field of
/// the whole data carries that name; a payload field sharing a common field's
/// spelling would otherwise let the case fact tighten the wrong reads.
fn field_name_unique(program: &TypedTrees, owner: &DataDefinition, name: &str) -> bool {
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

//! Complete constructed values for both contract operands and unfolded bodies.
//!
//! An omitted runtime field denotes its established zero value, not a wildcard.
//! Walk the selected declaration's whole roster so comparison cannot lose that
//! obligation. Erased terms are supplied by typing's accessible-constructor
//! elaboration; this layer never invents proof inhabitants. Case classifiers
//! used by arm guards remain separate from this value-construction route.

use numerics::bignum::BigInt;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataField, DataMember, DataVariant};
use typed_trees::expression::{ExpressionHandle, TableStructLiteral};
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

use super::StructuralTerm;

/// Exclusion test only: a retained membership occurrence must never be read as
/// a value equation. Proving the tag fact still requires the exact subject,
/// carrier and case checks in `has_exact_case_membership_meaning`.
pub(in super::super) fn is_case_observation(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    matches!(program.expression_table.expression(expression), typed_trees::expression::ExpressionNode::Binary(binary) if binary.operator == typed_trees::expression::BinaryOperator::CaseMembership)
        || program
            .expression_table
            .authored_selection_occurrences(expression)
            .any(|occurrence| {
                program
                    .authored_declaration_selections()
                    .get(occurrence)
                    .is_some_and(|selection| {
                        selection.kind()
                            == typed_trees::AuthoredDeclarationSelectionKind::CaseMembership
                    })
            })
}

pub(in super::super) fn case_guard_classifier<'program>(
    program: &'program TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
    expression: ExpressionHandle,
) -> Option<(&'program DataDefinition, &'program DataVariant)> {
    use typed_trees::expression::{BinaryOperator, ExpressionNode};
    let ExpressionNode::Binary(comparison) = program.expression_table.expression(expression) else {
        return None;
    };
    let (definition, variant) = case_classifier(program, comparison.right)?;
    let membership =
        crate::proof_contracts::bound_expression_meaning::has_exact_case_membership_meaning(
            program, machine, state, expression, comparison,
        );
    // Ordinary equality is a tag test only for an actually fieldless value.
    // A payload-free case can still carry common fields.
    let fieldless_equality = comparison.operator == BinaryOperator::Equal
        && !is_case_observation(program, expression)
        && constructor_fields(program, definition, Some(variant)).is_empty();
    (membership || fieldless_equality).then_some((definition, variant))
}

/// A case classifier is not a value. Arm matching and membership inspect only
/// these exact declarations; value consumers must instead construct its fields.
pub(in super::super) fn case_classifier(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(&DataDefinition, &DataVariant)> {
    let definition = crate::proof_contracts::bound_expression_meaning::exact_case_reference_owner(
        program, expression,
    )?;
    let typed_trees::expression::ExpressionNode::Name(path) =
        program.expression_table.expression(expression)
    else {
        return None;
    };
    let variant = program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(variant) if variant.symbol == path.symbol => Some(variant),
            _ => None,
        })?;
    Some((definition, variant))
}

pub(in super::super) fn case_value_term(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<StructuralTerm> {
    let (definition, variant) = case_classifier(program, expression)?;
    if !program.data_payload_fields(variant).is_empty() {
        return None;
    }
    if !definition.where_facts.is_empty() || !variant.where_facts.is_empty() {
        return None;
    }
    let fields = constructor_fields(program, definition, Some(variant))
        .into_iter()
        .map(|field| {
            if field.relevance.is_erased() {
                return None;
            }
            Some((
                field.name.as_str().to_owned(),
                zero_value_structural_term(program, field.type_reference)?,
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(constructed_term(definition, Some(variant), fields))
}

pub(in super::super) fn constructor_literal_term(
    program: &TypedTrees,
    literal: &TableStructLiteral,
    mut supplied_term: impl FnMut(ExpressionHandle) -> Option<StructuralTerm>,
) -> Option<StructuralTerm> {
    let definition = program.data_definitions().iter().find(|definition| {
        literal.type_symbol.is_valid() && definition.symbol == literal.type_symbol
    })?;
    let variant = match literal.case_symbol {
        Some(symbol) => Some(program.data_members(definition).iter().find_map(
            |member| match member {
                DataMember::Variant(variant) if symbol.is_valid() && variant.symbol == symbol => {
                    Some(variant)
                }
                _ => None,
            },
        )?),
        None => {
            if literal.case_name.is_some()
                || program
                    .data_members(definition)
                    .iter()
                    .any(|member| matches!(member, DataMember::Variant(_)))
            {
                return None;
            }
            None
        }
    };
    let declared = constructor_fields(program, definition, variant);
    let supplied = program.expression_table.struct_fields(literal.fields);
    // Field omission must not silently establish an owner/case predicate.
    // Explicit construction keeps its ordinary formation checks; defaults
    // need positive predicate evidence before this normalizer can fill them.
    if declared.len() != supplied.len()
        && (!definition.where_facts.is_empty()
            || variant.is_some_and(|variant| !variant.where_facts.is_empty()))
    {
        return None;
    }
    // No field may disappear, occur twice, or be rebound by its spelling.
    if supplied.iter().any(|field| {
        !declared
            .iter()
            .any(|declared| field.field_symbol.is_valid() && declared.symbol == field.field_symbol)
    }) {
        return None;
    }
    let mut fields = Vec::with_capacity(declared.len());
    for field in declared {
        let mut matches = supplied
            .iter()
            .filter(|supplied| supplied.field_symbol == field.symbol);
        let value = if let Some(supplied) = matches.next() {
            if matches.next().is_some() {
                return None;
            }
            supplied_term(supplied.value)?
        } else {
            if field.relevance.is_erased() {
                return None;
            }
            zero_value_structural_term(program, field.type_reference)?
        };
        fields.push((field.name.as_str().to_owned(), value));
    }
    Some(constructed_term(definition, variant, fields))
}

fn constructor_fields<'program>(
    program: &'program TypedTrees,
    definition: &'program DataDefinition,
    variant: Option<&'program DataVariant>,
) -> Vec<&'program DataField> {
    program
        .data_members(definition)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(field),
            DataMember::Variant(_) => None,
        })
        .chain(
            variant
                .into_iter()
                .flat_map(|variant| program.data_payload_fields(variant)),
        )
        .collect()
}

fn constructed_term(
    definition: &DataDefinition,
    variant: Option<&DataVariant>,
    mut fields: Vec<(String, StructuralTerm)>,
) -> StructuralTerm {
    fields.sort_by(|(left, _), (right, _)| left.cmp(right));
    StructuralTerm::Constructor {
        data: definition.name.as_str().to_owned(),
        case: variant
            .map(|variant| variant.name.as_str())
            .unwrap_or("")
            .to_owned(),
        fields,
    }
}

/// Storage zero is not implicit proof inhabitance. Only supported runtime
/// values whose complete default domain admits zero can enter this route.
pub(in super::super) fn zero_value_structural_term(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<StructuralTerm> {
    if !type_reference.is_valid()
        || typed_trees::proof_only::classify(program)
            .proof_only_mention(program, type_reference)
            .is_some()
    {
        return None;
    }
    zero_term(program, type_reference, &mut Vec::new())
}

fn zero_term(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    active: &mut Vec<SymbolHandle>,
) -> Option<StructuralTerm> {
    if crate::value_custody::data::type_requires_establishment(program, type_reference) {
        return None;
    }
    if let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(type_reference)
    {
        for constraint in program.type_reference_table.constraints(*constraints) {
            match constraint {
                TypeConstraintNode::ArithmeticDomain(_) => {}
                TypeConstraintNode::Range {
                    minimum,
                    maximum,
                    end_inclusive,
                } => {
                    // Check each range, not a partial intersection which may
                    // silently omit an uncomputed bound.
                    let minimum = crate::closed_integer_range_bound(program, *minimum)?;
                    let maximum =
                        crate::closed_integer_range_maximum(program, *maximum, *end_inclusive)?;
                    if minimum > BigInt::zero() || maximum < BigInt::zero() {
                        return None;
                    }
                }
                // These need their own established predicate evidence; peeling
                // the wrapper would silently waive the field's qualification.
                TypeConstraintNode::Domain(_) | TypeConstraintNode::Named(_) => return None,
            }
        }
        return zero_term(program, *base_type, active);
    }
    if let Some(primitive) = program.type_reference_table.primitive_type(type_reference) {
        return match primitive {
            PrimitiveType::Bool => Some(StructuralTerm::Constructor {
                data: "bool".to_owned(),
                case: "false".to_owned(),
                fields: Vec::new(),
            }),
            primitive if primitive.accepts_integer_literal() => {
                Some(StructuralTerm::Integer(BigInt::zero()))
            }
            _ => None,
        };
    }
    let symbol = match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { symbol, .. } => *symbol,
        // A generic first case may be independent of its arguments (None).
        // An actually stored open parameter still fails below: no declaration
        // or established zero value exists for that field type.
        TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
        _ => return None,
    };
    if !symbol.is_valid() || active.contains(&symbol) {
        return None;
    }
    let definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol)?;
    if crate::value_custody::data::data_requires_establishment(program, definition) {
        return None;
    }
    let variant = program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            DataMember::Field(_) => None,
        });
    // `zero_gated` is not a positive certificate for the selected case's
    // predicates. Their general instantiation/proof belongs to establishment;
    // until it can supply that evidence here, no implicit default may waive
    // them or choose a later, satisfiable case instead.
    if !definition.where_facts.is_empty()
        || variant.is_some_and(|variant| !variant.where_facts.is_empty())
    {
        return None;
    }
    active.push(symbol);
    let fields = constructor_fields(program, definition, variant)
        .into_iter()
        .map(|field| {
            if field.relevance.is_erased() {
                return None;
            }
            Some((
                field.name.as_str().to_owned(),
                zero_term(program, field.type_reference, active)?,
            ))
        })
        .collect::<Option<Vec<_>>>();
    active.pop();
    Some(constructed_term(definition, variant, fields?))
}

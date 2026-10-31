//! Complete constructed values for both contract operands and unfolded bodies.
//!
//! An omitted runtime field denotes its established zero value, not a wildcard.
//! Walk the selected declaration's whole roster so comparison cannot lose that
//! obligation. Erased terms are supplied by typing's accessible-constructor
//! elaboration; this layer never invents proof inhabitants. Case classifiers
//! used by arm guards remain separate from this value-construction route.

use crate::frozen_program::expression_table;
use numerics::bignum::BigInt;
use symbols::SymbolHandle;
use typed_trees::data::{DataDefinition, DataField, DataMember, DataVariant};
use typed_trees::expression::{ExpressionHandle, TableStructLiteral, TableStructLiteralField};
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

use super::StructuralTerm;

/// Read an already carrier-validated canonical value in the structural term
/// vocabulary. Encoded names retain no declaration authority here: the static
/// application intake must first replay the value against its resolved carrier.
pub(in super::super) fn admitted_const_value_term(
    value: &language_semantics::const_value::CanonicalConstIdentity,
) -> Option<StructuralTerm> {
    fn decoded(
        value: language_semantics::const_value::DecodedCanonicalConstValue,
    ) -> Option<StructuralTerm> {
        use language_semantics::const_value::DecodedCanonicalConstValue;
        let (data, case, fields) = match value {
            DecodedCanonicalConstValue::Integer { value, .. } => {
                return Some(StructuralTerm::Integer(BigInt::from_i128(value)));
            }
            DecodedCanonicalConstValue::Boolean(value) => {
                return Some(StructuralTerm::Constructor {
                    data: "bool".to_owned(),
                    case: value.to_string(),
                    fields: Vec::new(),
                });
            }
            DecodedCanonicalConstValue::Record { type_name, fields } => {
                (type_name, String::new(), fields)
            }
            DecodedCanonicalConstValue::Variant {
                type_name,
                case_name,
                fields,
            } => (type_name, case_name, fields),
            // These values have separate observation vocabularies; an encoded
            // array or float is not a record constructor or an integer atom.
            DecodedCanonicalConstValue::Array { .. } | DecodedCanonicalConstValue::Float { .. } => {
                return None;
            }
        };
        let mut fields = fields
            .into_iter()
            .map(|(name, value)| Some((name, decoded(value)?)))
            .collect::<Option<Vec<_>>>()?;
        fields.sort_by(|(left, _), (right, _)| left.cmp(right));
        Some(StructuralTerm::Constructor { data, case, fields })
    }

    decoded(value.decode_encoding()?)
}

/// Exclusion test only: a retained membership occurrence must never be read as
/// a value equation. Proving the tag fact still requires the exact subject,
/// carrier and case checks in `has_exact_case_membership_meaning`.
pub(in super::super) fn is_case_observation(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    matches!(expression_table(program).expression(expression), typed_trees::expression::ExpressionNode::Binary(binary) if binary.operator == typed_trees::expression::BinaryOperator::CaseMembership)
        || expression_table(program)
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
    program: &'program dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
    expression: ExpressionHandle,
) -> Option<(&'program DataDefinition, &'program DataVariant)> {
    use typed_trees::expression::BinaryOperator;
    let Some(comparison) = expression_table(program).binary_node(expression) else {
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
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<(&DataDefinition, &DataVariant)> {
    let definition = crate::proof_contracts::bound_expression_meaning::exact_case_reference_owner(
        program, expression,
    )?;
    let typed_trees::expression::ExpressionNode::Name(path) =
        expression_table(program).expression(expression)
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
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<StructuralTerm> {
    let (definition, variant) = case_classifier(program, expression)?;
    if !program.data_payload_fields(variant).is_empty() {
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
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    literal: &TableStructLiteral,
    mut field_term: impl FnMut(&DataField, Option<&TableStructLiteralField>) -> Option<StructuralTerm>,
) -> Option<StructuralTerm> {
    // Every consumer supplies the original construction occurrence. The
    // receiving application owns both its case and complete field carriers;
    // a template's field types cannot decide omitted zeros for an instance.
    let definition = crate::value_custody::struct_literals::selected_construction_definition(
        program, expression, literal,
    )?;
    let variant = match literal.case_symbol {
        Some(_) => Some(
            crate::value_custody::struct_literals::selected_construction_case(
                program, expression, literal, definition,
            )?,
        ),
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
    let supplied = expression_table(program).struct_fields(literal.fields);
    let selected_fields = supplied
        .iter()
        .map(|field| {
            crate::value_custody::struct_literals::selected_construction_field(
                program,
                expression,
                literal,
                field.field_symbol,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    // This forms a value term, not a predicate certificate. Source admission
    // independently checks the owner and selected case at their operand entry,
    // before this value exists. An omitted field still needs its own established
    // zero; the enclosing clause neither blocks normalization nor supplies it.
    // No field may disappear, occur twice, or be rebound by its spelling.
    if selected_fields.iter().any(|field| {
        !declared
            .iter()
            .any(|declared| field.symbol.is_valid() && declared.symbol == field.symbol)
    }) {
        return None;
    }
    complete_constructor_term(definition, variant, declared.into_iter(), |field| {
        let mut matches =
            supplied
                .iter()
                .zip(&selected_fields)
                .filter_map(|(supplied, selected)| {
                    (selected.symbol == field.symbol).then_some(supplied)
                });
        let value = if let Some(supplied) = matches.next() {
            if matches.next().is_some() {
                return None;
            }
            field_term(field, Some(supplied))?
        } else {
            if field.relevance.is_erased()
                || crate::value_custody::data::type_requires_establishment(
                    program,
                    field.type_reference,
                )
                || crate::proof_only_classification(program)
                    .proof_only_mention(program, field.type_reference)
                    .is_some()
            {
                return None;
            }
            // A completed runtime value may retain an opaque field identity
            // when this independently established zero has no mathematical
            // denotation. Pure term callers decline that observation.
            zero_value_structural_term(program, field.type_reference)
                .or_else(|| field_term(field, None))?
        };
        Some(value)
    })
}

/// A complete unqualified value snapshot from exact declared field captures.
/// This creates no predicate certificate, omitted zero, or erased inhabitant.
/// Construction and restoration supply their own independently formed inputs.
pub(in super::super) fn captured_constructor_term(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    variant: Option<&DataVariant>,
    field_value: impl FnMut(&DataField) -> Option<StructuralTerm>,
) -> Option<StructuralTerm> {
    complete_constructor_term(
        definition,
        variant,
        constructor_fields(program, definition, variant).into_iter(),
        field_value,
    )
}

fn complete_constructor_term<'field>(
    definition: &DataDefinition,
    variant: Option<&DataVariant>,
    declared: impl ExactSizeIterator<Item = &'field DataField>,
    mut field_value: impl FnMut(&DataField) -> Option<StructuralTerm>,
) -> Option<StructuralTerm> {
    let mut fields = Vec::with_capacity(declared.len());
    for field in declared {
        fields.push((field.name.as_str().to_owned(), field_value(field)?));
    }
    Some(constructed_term(definition, variant, fields))
}

fn constructor_fields<'program>(
    program: &'program dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<StructuralTerm> {
    if !type_reference.is_valid()
        || crate::proof_only_classification(program)
            .proof_only_mention(program, type_reference)
            .is_some()
    {
        return None;
    }
    zero_term(program, type_reference, &mut Vec::new())
}

fn zero_term(
    program: &dyn crate::CheckingInput,
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
            PrimitiveType::F32 | PrimitiveType::F64 => Some(StructuralTerm::Float(
                numerics::literals::FloatLiteral::from_f64(0.0).with_landing(
                    if primitive == PrimitiveType::F32 {
                        numerics::literals::FloatFormat::F32
                    } else {
                        numerics::literals::FloatFormat::F64
                    },
                ),
            )),
            primitive if primitive.accepts_integer_literal() => {
                Some(StructuralTerm::Integer(BigInt::zero()))
            }
            _ => None,
        };
    }
    if let TypeReferenceNode::FixedArray {
        element_type,
        length: typed_trees::types::FixedArrayLength::Literal(length),
    } = program.type_reference_table.type_reference(type_reference)
    {
        // Admission above independently established the zero representation.
        // Retain one element term, never allocate one term per array cell.
        let element = if *length == 0 {
            None
        } else {
            Some(Box::new(zero_term(program, *element_type, active)?))
        };
        return Some(StructuralTerm::RepeatedArray {
            length: u64::try_from(*length).ok()?,
            element,
        });
    }
    // A generic first case may be independent of its arguments (None). An
    // actually stored open parameter still fails below: no declaration or
    // established zero value exists for that field type.
    let symbol = program
        .type_reference_table
        .type_head_symbol(type_reference)?;
    if !symbol.is_valid() || active.contains(&symbol) {
        return None;
    }
    let definition = crate::declarations::data_queries::data_definition_by_symbol(program, symbol)?;
    if crate::value_custody::data::data_requires_establishment(program, definition) {
        return None;
    }
    active.push(symbol);
    let value = zero_constructor(program, definition, active);
    active.pop();
    value
}

/// The fixed first case and established zero children, before the owner's
/// predicate is admitted. This is an operand valuation, not evidence that the
/// containing value satisfies its default domain.
pub(in super::super) fn zero_constructor_operands(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
) -> Option<StructuralTerm> {
    zero_constructor(program, definition, &mut vec![definition.symbol])
}

fn zero_constructor(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    active: &mut Vec<SymbolHandle>,
) -> Option<StructuralTerm> {
    let variant = program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            DataMember::Field(_) => None,
        });
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
    Some(constructed_term(definition, variant, fields?))
}

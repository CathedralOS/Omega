//! Complete constructed values for both contract operands and unfolded bodies.
//!
//! An omitted runtime field denotes its established zero value, not a wildcard.
//! Walk the selected declaration's whole roster so comparison cannot lose that
//! obligation. Erased terms are supplied by typing's accessible-constructor
//! elaboration; this layer never invents proof inhabitants. Case classifiers
//! used by arm guards remain separate from this value-construction route.

use crate::frozen_program::expression_table;
use numerics::bignum::BigInt;
use numerics::literals::FloatLiteral;
use symbols::SymbolHandle;
use typed_trees::data::{DataDefinition, DataField, DataMember, DataVariant};
use typed_trees::expression::ExpressionNode;
use typed_trees::expression::{ExpressionHandle, TableStructLiteral, TableStructLiteralField};
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

use super::super::structural_judgment::ConstructorDeclaration;
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
                    declaration: None,
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
        Some(StructuralTerm::Constructor {
            declaration: None,
            data,
            case,
            fields,
        })
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
    matches!(expression_table(program).expression(expression), ExpressionNode::Binary(binary) if binary.operator == typed_trees::expression::BinaryOperator::CaseMembership)
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
    let comparison = expression_table(program).binary_node(expression)?;
    let (definition, variant) = case_classifier(program, comparison.right)?;
    let membership =
        crate::proof_contracts::bound_expression_meaning::has_exact_case_membership_meaning(
            program, machine, state, expression, comparison,
        );
    // Ordinary equality is a tag test only for an actually fieldless value.
    // A payload-free case can still carry common fields.
    let fieldless_equality = comparison.operator == BinaryOperator::Equal
        && !is_case_observation(program, expression)
        && crate::has_builtin_binary_expression_meaning(program, machine, state, expression)
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
    let ExpressionNode::Name(path) = expression_table(program).expression(expression) else {
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

pub(in super::super) fn constructed_term(
    definition: &DataDefinition,
    variant: Option<&DataVariant>,
    mut fields: Vec<(String, StructuralTerm)>,
) -> StructuralTerm {
    fields.sort_by(|(left, _), (right, _)| left.cmp(right));
    StructuralTerm::Constructor {
        declaration: Some(ConstructorDeclaration {
            data: definition.symbol,
            case: variant.map(|variant| variant.symbol),
        }),
        data: definition.name.as_str().to_owned(),
        case: variant
            .map(|variant| variant.name.as_str())
            .unwrap_or("")
            .to_owned(),
        fields,
    }
}

/// Retain a known constructor's declarations. Old encoded terms have only
/// names, so they may acquire this association only when exactly one owner
/// exists. A missing or invalid retained declaration never falls back to names.
pub(in super::super) fn constructor_declaration(
    program: &dyn crate::CheckingInput,
    declaration: Option<ConstructorDeclaration>,
    data: &str,
    case: &str,
) -> Option<ConstructorDeclaration> {
    // Builtin Boolean terms have their own meaning and no nominal data row.
    if declaration.is_none() && data == "bool" {
        return None;
    }
    let definition = if let Some(declaration) = declaration {
        program.data_definition_by_symbol(declaration.data)?
    } else {
        let mut definitions = program
            .data_definitions()
            .iter()
            .filter(|definition| definition.name.as_str() == data);
        let definition = definitions.next()?;
        if definitions.next().is_some() {
            return None;
        }
        definition
    };
    if definition.name.as_str() != data {
        return None;
    }
    let case_symbol = if case.is_empty() {
        if program
            .data_members(definition)
            .iter()
            .any(|member| matches!(member, DataMember::Variant(_)))
        {
            return None;
        }
        None
    } else {
        Some(program.data_members(definition).iter().find_map(|member| {
            match member {
                DataMember::Variant(variant)
                    if variant.name.as_str() == case
                        && declaration
                            .is_none_or(|declaration| declaration.case == Some(variant.symbol)) =>
                {
                    Some(variant.symbol)
                }
                _ => None,
            }
        })?)
    };
    let selected = ConstructorDeclaration {
        data: definition.symbol,
        case: case_symbol,
    };
    declaration
        .is_none_or(|declaration| declaration == selected)
        .then_some(selected)
}

/// Constructor payload comparison requires the same declared data owner.
/// Primitive Boolean terms have no data declaration; their builtin meaning is
/// retained separately from nominal data constructors.
pub(in super::super) fn same_constructor_data(
    program: &dyn crate::CheckingInput,
    left: &StructuralTerm,
    right: &StructuralTerm,
) -> bool {
    let (
        StructuralTerm::Constructor {
            declaration: left_declaration,
            data: left_data,
            case: left_case,
            ..
        },
        StructuralTerm::Constructor {
            declaration: right_declaration,
            data: right_data,
            case: right_case,
            ..
        },
    ) = (left, right)
    else {
        return false;
    };
    if left_declaration.is_none()
        && right_declaration.is_none()
        && left_data == "bool"
        && right_data == "bool"
    {
        return true;
    }
    let (Some(left), Some(right)) = (
        constructor_declaration(program, *left_declaration, left_data, left_case),
        constructor_declaration(program, *right_declaration, right_data, right_case),
    ) else {
        return false;
    };
    left.data == right.data
}

/// A body can retain its authored template classifier while the actual value
/// retains a closed application. Their existing constructor correspondence
/// translates that case; a sibling application or same name supplies nothing.
pub(in super::super) fn constructor_matches_case(
    program: &dyn crate::CheckingInput,
    subject: &StructuralTerm,
    data: SymbolHandle,
    case: SymbolHandle,
) -> bool {
    let StructuralTerm::Constructor {
        declaration,
        data: name,
        case: case_name,
        ..
    } = subject.semantic_value()
    else {
        return false;
    };
    let Some(selected) = constructor_declaration(program, *declaration, name, case_name) else {
        return false;
    };
    if selected.data == data {
        return selected.case == Some(case);
    }
    let Some(reference) = program
        .data_definition_by_symbol(selected.data)
        .and_then(|definition| definition.generic_instance)
    else {
        return false;
    };
    program
        .data_constructor_definition(reference, data)
        .is_some_and(|definition| definition.symbol == selected.data)
        && program
            .data_constructor_case(reference, data, case)
            .is_some_and(|variant| selected.case == Some(variant.symbol))
}

/// Storage zero is not implicit proof inhabitance. Only supported runtime
/// values whose complete default domain admits zero can enter this route.
pub(in super::super) fn zero_value_structural_term(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<StructuralTerm> {
    if !type_reference.is_valid() {
        return None;
    }
    if crate::proof_only_classification(program)
        .proof_only_mention(program, type_reference)
        .is_some()
    {
        trace_zero_value(program, type_reference, None, "proof-only-carrier");
        return None;
    }
    let value = zero_term(program, type_reference, &mut Vec::new());
    trace_zero_value(program, type_reference, value.as_ref(), "completed-query");
    value
}

/// A failed zero-value proof must identify the declaration actually available
/// in this checking context. Read only retained metadata and the query's own
/// result; tracing must not perform another proof or establish missing storage.
fn trace_zero_value(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    value: Option<&StructuralTerm>,
    reason: &str,
) {
    if !super::super::struct_trace() {
        return;
    }
    let head = program.type_reference_table.type_head_symbol(reference);
    let declaration = head.and_then(|symbol| {
        program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == symbol)
    });
    let members = declaration.map(|data| {
        program
            .data_members(data)
            .iter()
            .take(8)
            .map(|member| match member {
                DataMember::Field(field) => ("field", field.name.as_str(), field.symbol, 0),
                DataMember::Variant(case) => (
                    "case",
                    case.name.as_str(),
                    case.symbol,
                    program.data_payload_fields(case).len(),
                ),
            })
            .collect::<Vec<_>>()
    });
    let result = match value {
        Some(StructuralTerm::Constructor {
            data, case, fields, ..
        }) => Some((data.as_str(), case.as_str(), fields.len())),
        Some(_) => Some(("<non-constructor>", "", 0)),
        None => None,
    };
    eprintln!(
        "STRUCT zero-value-query reason={reason} reference={reference:?} type={:?} head={head:?} declaration={:?} members={members:?} result={result:?}",
        program.type_reference_table.type_reference(reference),
        declaration.map(|data| (
            data.symbol,
            data.name.as_str(),
            data.type_parameters,
            data.generic_instance,
            data.zero_gated
        )),
    );
}

fn zero_term(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    active: &mut Vec<SymbolHandle>,
) -> Option<StructuralTerm> {
    if crate::value_custody::data::type_requires_establishment(program, type_reference) {
        trace_zero_value(
            program,
            type_reference,
            None,
            "storage-requires-establishment",
        );
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
                declaration: None,
                data: "bool".to_owned(),
                case: "false".to_owned(),
                fields: Vec::new(),
            }),
            PrimitiveType::F32 | PrimitiveType::F64 => Some(StructuralTerm::Float(
                FloatLiteral::from_f64(0.0).with_landing(if primitive == PrimitiveType::F32 {
                    numerics::literals::FloatFormat::F32
                } else {
                    numerics::literals::FloatFormat::F64
                }),
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
    let Some(symbol) = program
        .type_reference_table
        .type_head_symbol(type_reference)
    else {
        trace_zero_value(program, type_reference, None, "missing-type-head");
        return None;
    };
    if !symbol.is_valid() || active.contains(&symbol) {
        trace_zero_value(
            program,
            type_reference,
            None,
            "unresolved-or-recursive-type",
        );
        return None;
    }
    let Some(definition) =
        crate::declarations::data_queries::data_definition_by_symbol(program, symbol)
    else {
        trace_zero_value(program, type_reference, None, "missing-data-declaration");
        return None;
    };
    if crate::value_custody::data::data_requires_establishment(program, definition) {
        trace_zero_value(program, type_reference, None, "data-requires-establishment");
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

use crate::source_state::TypedTrees;
use crate::source_state::data::{DataDefinition, DataField, DataVariant};
use crate::source_state::expression::{
    ExpressionNode, TableCastExpression, TableStructLiteral, TableStructLiteralField,
};
use crate::source_state::types::{
    DomainConstraint, DomainConstraintSubject, PrimitiveType, TypeConstraintNode,
    TypeReferenceHandle, TypeReferenceNode,
};
use application_identity::const_value::DecodedCanonicalConstValue;
use language_core::cast_form::{CastEntry, CastEntryKind, CastForm};
use numeric_literals::{FloatFormat, FloatLiteral, IntegerLanding, IntegerLiteral, IntegerRadix};

/// The declared domains a carrier qualifies its value with, innermost
/// first, and the carrier beneath them.
///
/// An evaluated snapshot holds a value's bits and declared carrier, not
/// the evidence its invocation checked for each domain on it. The
/// materializer re-establishes each domain with an `as +D` entry, so the
/// reader of the materialized value owes that domain's ordinary issuer
/// and predicate checks (domains.md, "Domain-only casts"); the
/// evaluator refuses a domain whose issuance is restricted before this
/// point, since a constant grants no authority (constants.md). An
/// arithmetic policy stays on the carrier, where integer landing reads
/// it; any other constraint, alone or beside a declared domain, has no
/// materialization and yields `None`.
pub(crate) fn declared_domain_qualifications(
    program: &TypedTrees,
    carrier: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, Vec<&DomainConstraint>)> {
    let mut base = carrier;
    let mut outermost_first = Vec::new();
    while base.is_valid() {
        let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = program.type_reference_node(base)
        else {
            break;
        };
        let constraints = program.type_reference_table.constraints(*constraints);
        if constraints
            .iter()
            .all(|constraint| matches!(constraint, TypeConstraintNode::ArithmeticDomain(_)))
        {
            break;
        }
        for constraint in constraints.iter().rev() {
            let TypeConstraintNode::Domain(domain) = constraint else {
                return None;
            };
            if domain.subject != DomainConstraintSubject::Declared || !domain.symbol.is_valid()
            {
                return None;
            }
            outermost_first.push(domain);
        }
        base = *base_type;
    }
    outermost_first.reverse();
    Some((base, outermost_first))
}

pub(crate) fn materialize(
    program: &mut TypedTrees,
    declared_type: TypeReferenceHandle,
    value: &DecodedCanonicalConstValue,
) -> Option<ExpressionNode> {
    let (base, domains) = declared_domain_qualifications(program, declared_type)?;
    if domains.is_empty() {
        return materialize_carrier(program, declared_type, value);
    }
    let domains = domains.into_iter().cloned().collect::<Vec<_>>();
    let mut node = materialize_carrier(program, base, value)?;
    let mut input_type = base;
    for (position, domain) in domains.iter().enumerate() {
        // Each entry adds one domain; `target_type` carries the input type
        // before this entry, and `result_type` carries the domain added here
        // along with every one added beneath it.
        let result_type = if position + 1 == domains.len() {
            declared_type
        } else {
            let constraints = program.type_reference_table.insert_constraints(
                domains[..=position]
                    .iter()
                    .cloned()
                    .map(TypeConstraintNode::Domain),
            );
            program
                .type_reference_table
                .insert(TypeReferenceNode::Constrained {
                    base_type: base,
                    constraints,
                })
        };
        node = ExpressionNode::Cast(TableCastExpression {
            value: program.expression_table.insert(node),
            target_type: input_type,
            result_type,
            target_label: arena::HandleSpan::empty(),
            domain: numerics::arithmetic::ArithmeticDomain::Exact,
            semantic_domain: arena::HandleSpan::empty(),
            semantic_domain_arguments: arena::HandleSpan::empty(),
            semantic_domain_symbol: domain.symbol,
            semantic_domain_id: domain.semantic_id,
            form: added(position == 0),
        });
        input_type = result_type;
    }
    Some(node)
}

fn added(first: bool) -> CastForm {
    CastForm::Entry(CastEntry {
        kind: CastEntryKind::Add,
        first,
        after_carrier: false,
    })
}

fn materialize_carrier(
    program: &mut TypedTrees,
    declared_type: TypeReferenceHandle,
    value: &DecodedCanonicalConstValue,
) -> Option<ExpressionNode> {
    match value {
        DecodedCanonicalConstValue::StaticCallable { .. } => None,
        DecodedCanonicalConstValue::Float { format, bits } => {
            (float_format(program, declared_type)? == *format).then_some(())?;
            Some(ExpressionNode::Float(FloatLiteral::from_bits(
                *format, *bits,
            )?))
        }
        DecodedCanonicalConstValue::Integer {
            integer_type,
            value,
        } => {
            let primitive = program.primitive_type_reference(declared_type)?;
            if Some(*integer_type) != primitive.landed_landing() {
                return None;
            }
            let literal = IntegerLiteral::from_parts(
                value.is_negative(),
                IntegerRadix::Decimal,
                &value.abs().to_string(),
            )
            .ok()?;
            integer_value(program, declared_type, literal)
        }
        DecodedCanonicalConstValue::Boolean(value) => {
            (program.primitive_type_reference(declared_type) == Some(PrimitiveType::Bool))
                .then_some(ExpressionNode::Boolean(*value))
        }
        DecodedCanonicalConstValue::Array { type_name, values } => {
            if *type_name != program.display_type_reference(declared_type) {
                return None;
            }
            let declared_type = program.unwrapped_type_reference(declared_type)?;
            let TypeReferenceNode::FixedArray {
                element_type,
                ..
            } = program.type_reference_node(declared_type).clone()
            else {
                return None;
            };
            let length = program.closed_fixed_array_length(declared_type)?;
            if length != values.len() {
                return None;
            }
            let mut elements = Vec::with_capacity(length);
            for value in values {
                let element = materialize(program, element_type, value)?;
                elements.push(program.expression_table.insert(element));
            }
            Some(ExpressionNode::ArrayLiteral(
                program.expression_table.insert_expression_handles(elements),
            ))
        }
        DecodedCanonicalConstValue::Record { type_name, fields } => {
            record(program, declared_type, type_name, None, fields)
        }
        DecodedCanonicalConstValue::Variant {
            type_name,
            case_name,
            fields,
        } => record(program, declared_type, type_name, Some(case_name), fields),
    }
}

fn aggregate_declaration<'a>(
    program: &'a TypedTrees,
    carrier: TypeReferenceHandle,
    type_name: &str,
) -> Option<&'a DataDefinition> {
    let carrier = program.unwrapped_type_reference(carrier)?;
    // A layout storage application holds its semantic record's fields;
    // the literal names that record and is applied at the storage type.
    if let Some(storage) = program.layout_storage_arguments(carrier) {
        let declaration = program.closed_data_definition(storage.semantic_type)?;
        return (declaration.symbol.is_valid() && declaration.name.as_str() == type_name)
            .then_some(declaration);
    }
    let symbol = program.type_reference_node(carrier).as_named_symbol()?;
    let declaration = program.data_definition_by_symbol(symbol)?;
    (symbol.is_valid() && declaration.name.as_str() == type_name).then_some(declaration)
}

/// The type a constant's record literal is applied at: the storage
/// application when the carrier is one, so the literal constructs that
/// representation rather than the bare semantic record.
fn literal_application(
    program: &TypedTrees,
    carrier: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let underlying = program.unwrapped_type_reference(carrier)?;
    program
        .layout_storage_arguments(underlying)
        .map(|_| underlying)
}

fn named_case<'a>(
    program: &'a TypedTrees,
    declaration: &'a DataDefinition,
    name: &str,
) -> Option<&'a DataVariant> {
    let mut matching = program
        .data_members(declaration)
        .iter()
        .filter_map(|member| member.as_variant())
        .filter(|case| case.name.as_str() == name);
    let (Some(case), None) = (matching.next(), matching.next()) else {
        return None;
    };
    case.symbol.is_valid().then_some(case)
}

fn record(
    program: &mut TypedTrees,
    declared_type: TypeReferenceHandle,
    type_name: &str,
    case_name: Option<&str>,
    fields: &[(String, DecodedCanonicalConstValue)],
) -> Option<ExpressionNode> {
    let definition = aggregate_declaration(program, declared_type, type_name)?;
    let mut literal = TableStructLiteral {
        type_name: definition.name.clone(),
        type_symbol: definition.symbol,
        ..TableStructLiteral::default()
    };
    if let Some(application) = literal_application(program, declared_type) {
        literal.application_type = application;
    }
    let declared_fields = if let Some(case_name) = case_name {
        let variant = named_case(program, definition, case_name)?;
        literal.case_name = Some(variant.name.clone());
        literal.case_symbol = Some(variant.symbol);
        program.data_payload_fields(variant).to_vec()
    } else {
        program
            .data_members(definition)
            .iter()
            .map(|member| {
                let field = member.as_field()?;
                Some(field.clone())
            })
            .collect::<Option<Vec<_>>>()?
    };
    if declared_fields.len() != fields.len() {
        return None;
    }
    let mut materialized = Vec::with_capacity(fields.len());
    for (declared, (name, value)) in declared_fields.iter().zip(fields) {
        if declared.name.as_str() != name || !declared.symbol.is_valid() {
            return None;
        }
        let value = materialize(program, declared.type_reference, value)?;
        materialized.push(TableStructLiteralField {
            name: declared.name.clone(),
            field_symbol: declared.symbol,
            value: program.expression_table.insert(value),
        });
    }
    literal.fields = program.expression_table.insert_struct_fields(materialized);
    Some(ExpressionNode::StructLiteral(literal))
}

fn float_format(program: &TypedTrees, carrier: TypeReferenceHandle) -> Option<FloatFormat> {
    match program.primitive_type_reference(carrier)? {
        PrimitiveType::F32 => Some(FloatFormat::F32),
        PrimitiveType::F64 => Some(FloatFormat::F64),
        _ => None,
    }
}

fn integer_value(
    program: &TypedTrees,
    declared_type: TypeReferenceHandle,
    literal: IntegerLiteral,
) -> Option<ExpressionNode> {
    let primitive = program.primitive_type_reference(declared_type)?;
    let landed_type = primitive.landed_landing()?;
    Some(ExpressionNode::Integer(
        literal.with_landing(IntegerLanding {
            landed_type,
            domain: program
                .type_reference_table
                .arithmetic_domain(declared_type),
        }),
    ))
}

// Callable fields share the source files for this check; hash them once,
// only when a field needs them.
type ToolchainSources =
    std::cell::OnceCell<Result<Vec<(source_text::SourceId, [u8; 32])>, String>>;

fn matches_materialized(
    program: &TypedTrees,
    carrier: TypeReferenceHandle,
    expression: crate::source_state::expression::ExpressionHandle,
    value: &DecodedCanonicalConstValue,
    sources: &ToolchainSources,
) -> bool {
    if !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    let Some((base, domains)) = declared_domain_qualifications(program, carrier) else {
        return false;
    };
    if !domains.is_empty() {
        let mut expression = expression;
        for (position, domain) in domains.iter().enumerate().rev() {
            if !program.expression_table.expression_is_valid(expression) {
                return false;
            }
            let ExpressionNode::Cast(cast) = program.expression_table.expression(expression)
            else {
                return false;
            };
            if cast.form != added(position == 0)
                || cast.semantic_domain_symbol != domain.symbol
                || cast.semantic_domain_id != domain.semantic_id
                || (position + 1 == domains.len() && cast.result_type != carrier)
            {
                return false;
            }
            expression = cast.value;
        }
        return matches_materialized(program, base, expression, value, sources);
    }
    match (program.expression_table.expression(expression), value) {
        (
            ExpressionNode::MachineValue(actual),
            expected @ DecodedCanonicalConstValue::StaticCallable { .. },
        ) => {
            let Ok(sources) = sources.get_or_init(|| {
                source_admission::source_identity::toolchain_source_identities(
                    program.symbols.source_files(),
                )
            }) else {
                return false;
            };
            program.normalized_type_identity(actual.type_reference)
                == program.normalized_type_identity(carrier)
                && program
                    .canonical_static_callable_value(actual, sources)
                    .as_ref()
                    == Ok(expected)
        }
        (ExpressionNode::Boolean(actual), DecodedCanonicalConstValue::Boolean(expected)) => {
            actual == expected
        }
        (ExpressionNode::Float(actual), DecodedCanonicalConstValue::Float { format, bits }) => {
            float_format(program, carrier) == Some(*format)
                && actual.landing() == Some(*format)
                && *bits
                    == match format {
                        FloatFormat::F32 => u64::from(actual.f32_bits()),
                        FloatFormat::F64 => actual.value_f64().to_bits(),
                    }
        }
        (
            ExpressionNode::Integer(actual),
            DecodedCanonicalConstValue::Integer {
                integer_type,
                value,
            },
        ) => {
            actual.landing()
                == Some(IntegerLanding {
                    landed_type: *integer_type,
                    domain: program.type_reference_table.arithmetic_domain(carrier),
                })
                && actual.value_bignum().as_ref() == Some(value)
        }
        (
            ExpressionNode::ArrayLiteral(actual),
            DecodedCanonicalConstValue::Array { type_name, values },
        ) => {
            let Some(underlying) = program.unwrapped_type_reference(carrier) else {
                return false;
            };
            let TypeReferenceNode::FixedArray {
                element_type,
                ..
            } = program.type_reference_node(underlying)
            else {
                return false;
            };
            let expressions = program.expression_table.expression_handles(*actual);
            *type_name == program.display_type_reference(carrier)
                && program.closed_fixed_array_length(underlying) == Some(values.len())
                && expressions.len() == values.len()
                && expressions.iter().zip(values).all(|(expression, value)| {
                    matches_materialized(program, *element_type, *expression, value, sources)
                })
        }
        (ExpressionNode::StructLiteral(actual), value) => {
            let (type_name, case_name, fields) = match value {
                DecodedCanonicalConstValue::Record { type_name, fields } => {
                    (type_name, None, fields)
                }
                DecodedCanonicalConstValue::Variant {
                    type_name,
                    case_name,
                    fields,
                } => (type_name, Some(case_name.as_str()), fields),
                _ => return false,
            };
            let Some(declaration) = aggregate_declaration(program, carrier, type_name) else {
                return false;
            };
            if actual.type_symbol != declaration.symbol
                || literal_application(program, carrier)
                    .is_some_and(|application| actual.application_type != application)
            {
                return false;
            }
            let expressions = program.expression_table.struct_fields(actual.fields);
            if let Some(case_name) = case_name {
                let Some(case) = named_case(program, declaration, case_name) else {
                    return false;
                };
                actual.case_symbol == Some(case.symbol)
                    && actual.case_name.as_ref() == Some(&case.name)
                    && matches_fields(
                        program,
                        program.data_payload_fields(case).iter().map(Some),
                        expressions,
                        fields,
                        sources,
                    )
            } else {
                actual.case_symbol.is_none()
                    && actual.case_name.is_none()
                    && matches_fields(
                        program,
                        program
                            .data_members(declaration)
                            .iter()
                            .map(|member| member.as_field()),
                        expressions,
                        fields,
                        sources,
                    )
            }
        }
        _ => false,
    }
}

fn matches_fields<'a>(
    program: &TypedTrees,
    declared: impl ExactSizeIterator<Item = Option<&'a DataField>>,
    expressions: &[TableStructLiteralField],
    fields: &[(String, DecodedCanonicalConstValue)],
    sources: &ToolchainSources,
) -> bool {
    declared.len() == fields.len()
        && expressions.len() == fields.len()
        && declared
            .zip(expressions)
            .zip(fields)
            .all(|((field, expression), (name, value))| {
                field.is_some_and(|field| {
                    field.symbol == expression.field_symbol
                        && field.name == expression.name
                        && field.name.as_str() == name
                        && matches_materialized(
                            program,
                            field.type_reference,
                            expression.value,
                            value,
                            sources,
                        )
                })
            })
}

impl TypedTrees {
    pub fn canonical_const_materialization_matches(
        &self,
        carrier: TypeReferenceHandle,
        expression: crate::source_state::expression::ExpressionHandle,
        value: &DecodedCanonicalConstValue,
    ) -> bool {
        let sources = ToolchainSources::new();
        matches_materialized(self, carrier, expression, value, &sources)
    }

    /// The declared domains on a constant carrier, innermost first
    /// (`declared_domain_qualifications`).
    pub fn constant_domain_qualifications(
        &self,
        carrier: TypeReferenceHandle,
    ) -> Option<Vec<&DomainConstraint>> {
        declared_domain_qualifications(self, carrier).map(|(_, domains)| domains)
    }

    pub fn materialize_canonical_const_value(
        &mut self,
        carrier: TypeReferenceHandle,
        value: &DecodedCanonicalConstValue,
    ) -> Option<ExpressionNode> {
        materialize(self, carrier, value)
    }
}

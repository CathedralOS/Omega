//! Numeric substitution preserves the selected declaration's landing boundary.

use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::{FloatFormat, FloatLiteral, IntegerLanding, LandedIntegerType};
use source::SourceSpan;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
use symbol_resolved_trees::types::TypeReference;
use symbols::{BuiltinTypeAtom, SymbolHandle};

pub(super) fn retain_declared_carrier(
    program: &mut SymbolResolvedTrees,
    expression: ExpressionHandle,
    declaration_symbol: SymbolHandle,
    reference: SourceSpan,
) -> Result<(), Diagnostic> {
    let declaration = program
        .const_declarations
        .iter()
        .find(|declaration| declaration.symbol == declaration_symbol)
        .ok_or_else(|| Diagnostic::error("constant substitution lost its declared carrier"))?;
    let declared_type = declaration.declared_type.clone();
    retain_value_carrier(program, expression, &declared_type, reference)
}

fn retain_value_carrier(
    program: &mut SymbolResolvedTrees,
    expression: ExpressionHandle,
    declared_type: &TypeReference,
    reference: SourceSpan,
) -> Result<(), Diagnostic> {
    if let TypeReference::FixedArray(array) = declared_type {
        let ExpressionNode::ArrayLiteral(elements) =
            program.tables.bodies.expressions.expression(expression)
        else {
            return Ok(());
        };
        let elements = *elements;
        let element_type = program.child_type_reference(array.element_type).clone();
        for element_ordinal in 0..elements.count() {
            let element = *program
                .tables
                .bodies
                .expressions
                .expression_handle_at_offset(elements, element_ordinal);
            retain_value_carrier(program, element, &element_type, reference)?;
        }
        return Ok(());
    }
    let TypeReference::Named { symbol, .. } = declared_type else {
        return Ok(());
    };
    let Some(atom) = program.symbols.builtin_type_atom(*symbol) else {
        return Ok(());
    };
    let integer_type = match atom {
        BuiltinTypeAtom::I8 => Some(LandedIntegerType::I8),
        BuiltinTypeAtom::I16 => Some(LandedIntegerType::I16),
        BuiltinTypeAtom::I32 => Some(LandedIntegerType::I32),
        BuiltinTypeAtom::I64 => Some(LandedIntegerType::I64),
        BuiltinTypeAtom::U8 => Some(LandedIntegerType::U8),
        BuiltinTypeAtom::U16 => Some(LandedIntegerType::U16),
        BuiltinTypeAtom::U32 => Some(LandedIntegerType::U32),
        BuiltinTypeAtom::U64 => Some(LandedIntegerType::U64),
        BuiltinTypeAtom::Address => Some(LandedIntegerType::Addr),
        _ => None,
    };
    let float_format = match atom {
        BuiltinTypeAtom::F32 => Some(FloatFormat::F32),
        BuiltinTypeAtom::F64 => Some(FloatFormat::F64),
        _ => None,
    };
    let mismatch = || {
        Diagnostic::error(
            "constant initializer landing conflicts with its declared numeric carrier",
        )
        .with_source_span(reference)
    };
    let node = program.tables.bodies.expressions.expression_mut(expression);
    match node {
        ExpressionNode::Integer(literal) => {
            if let Some(landed_type) = integer_type {
                let landing = IntegerLanding {
                    landed_type,
                    domain: ArithmeticDomain::Exact,
                };
                if literal.landing().is_some_and(|actual| actual != landing) {
                    return Err(mismatch());
                }
                *literal = literal.with_landing(landing);
            } else if let Some(format) = float_format {
                if literal.landing().is_some() {
                    return Err(mismatch());
                }
                let value = literal.value_bignum().ok_or_else(mismatch)?;
                let float = FloatLiteral::parse(&value.to_string()).ok_or_else(mismatch)?;
                *node = ExpressionNode::Float(float.with_landing(format));
            }
        }
        ExpressionNode::Float(literal) => {
            if integer_type.is_some() {
                return Err(mismatch());
            }
            if let Some(format) = float_format {
                if literal.landing().is_some_and(|actual| actual != format) {
                    return Err(mismatch());
                }
                *literal = literal.with_landing(format);
            }
        }
        _ => {}
    }
    Ok(())
}

/// Nominal leaves require identity custody beyond their encoded value labels.
pub(super) fn has_nominal_carrier(program: &SymbolResolvedTrees, carrier: &TypeReference) -> bool {
    match carrier {
        TypeReference::Named { symbol, .. } => {
            program.symbols.get(*symbol).kind == symbols::SymbolKind::Data
        }
        TypeReference::FixedArray(array) => {
            has_nominal_carrier(program, program.child_type_reference(array.element_type))
        }
        TypeReference::Constrained(carrier) => {
            has_nominal_carrier(program, program.child_type_reference(carrier.base_type))
        }
        _ => false,
    }
}

/// Reconstruct equality from resolved declarations and array geometry, never
/// from rendered type names or equal structural initializer encodings.
pub(super) fn same_resolved_carrier(
    program: &SymbolResolvedTrees,
    left: &TypeReference,
    right: &TypeReference,
) -> bool {
    match (left, right) {
        (TypeReference::Named { symbol: left, .. }, TypeReference::Named { symbol: right, .. }) => {
            left.is_valid() && left == right
        }
        (TypeReference::FixedArray(left), TypeReference::FixedArray(right)) => {
            left.length == right.length
                && same_resolved_carrier(
                    program,
                    program.child_type_reference(left.element_type),
                    program.child_type_reference(right.element_type),
                )
        }
        _ => false,
    }
}

pub(super) fn encoding_has_nominal_carrier(encoding: &str) -> bool {
    use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
    fn contains(value: &DecodedCanonicalConstValue) -> bool {
        match value {
            DecodedCanonicalConstValue::Record { .. }
            | DecodedCanonicalConstValue::Variant { .. } => true,
            DecodedCanonicalConstValue::Array { values, .. } => values.iter().any(contains),
            _ => false,
        }
    }
    CanonicalConstValue::new("", encoding, "")
        .decode_encoding()
        .is_some_and(|value| contains(&value))
}

/// Rejoin the parent-owned child span in actual resolved type owners. Copies
/// of one application may share its span, but must agree on the resolved base.
/// No encoded name or detached provisional application supplies this identity.
pub(super) fn receiving_parameters(
    program: &SymbolResolvedTrees,
    arguments: arena::HandleSpan<TypeReference>,
) -> Option<&[symbol_resolved_trees::data::TypeParameter]> {
    let mut selected = SymbolHandle::invalid();
    let mut conflicting = false;
    let mut inspect = |reference: &TypeReference| {
        if let TypeReference::Generic(application) = reference
            && application.arguments == arguments
        {
            if !application.base_symbol.is_valid()
                || (selected.is_valid() && selected != application.base_symbol)
            {
                conflicting = true;
            }
            selected = application.base_symbol;
        }
    };
    let declarations = &program.tables.declarations;
    for (_, reference) in declarations.child_type_references.iter() {
        inspect(reference);
    }
    for (_, parameter) in declarations.state_parameters.iter() {
        inspect(&parameter.type_reference);
    }
    for (_, state) in declarations.machine_states.iter() {
        if let Some(reference) = &state.return_type {
            inspect(reference);
        }
    }
    for (_, member) in declarations.data_members.iter() {
        if let symbol_resolved_trees::data::DataMember::Field(field) = member {
            inspect(&field.type_reference);
        }
    }
    for (_, field) in declarations.data_payload_fields.iter() {
        inspect(&field.type_reference);
    }
    for (_, statement) in declarations.state_statements.iter() {
        if let symbol_resolved_trees::statement::Statement::LocalData(local) = statement {
            inspect(&local.type_reference);
        }
    }
    for definition in program.data_definitions.iter() {
        if let Some(reference) = &definition.generic_instance {
            inspect(reference);
        }
    }
    for (_, constraint) in program.tables.types.constraints.iter() {
        if let symbol_resolved_trees::types::TypeConstraint::Domain(domain) = constraint
            && domain.arguments == arguments
        {
            // Domain constraints retain an authored path at this phase. Resolve
            // that actual occurrence through the shared source-aware resolver;
            // its argument span, not a detached name copy, identifies the use.
            let domain_symbol = program
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    domain.name.as_str(),
                    &[symbols::SymbolKind::Domain],
                    domain.name.source_span(),
                )?;
            if selected.is_valid() && selected != domain_symbol {
                conflicting = true;
            }
            selected = domain_symbol;
        }
    }
    if conflicting || !selected.is_valid() {
        return None;
    }
    if let Some(template) = program
        .data_definitions
        .iter()
        .find(|definition| definition.symbol == selected)
    {
        return Some(program.data_type_parameters(template.type_parameters));
    }
    let domain = program
        .domain_definitions
        .iter()
        .find(|definition| definition.symbol == selected)?;
    let parameters = program.data_type_parameters(domain.type_parameters);
    let carrier_binder = parameters.first().is_some_and(|parameter| matches!(parameter.kind, symbol_resolved_trees::data::TypeParameterKind::Type)
        && matches!(&domain.target_type, TypeReference::Named { symbol, .. } if *symbol == parameter.symbol));
    Some(if carrier_binder {
        &parameters[1..]
    } else {
        parameters
    })
}

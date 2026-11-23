//! Decode and inspect typed constant declarations and arguments.
use crate::*;
pub fn canonical_const_declaration_value(
    program: &TypedTrees,
    symbol: symbols::SymbolHandle,
) -> Option<application_identity::const_value::CanonicalConstValue> {
    use application_identity::const_value::CanonicalConstValue;
    use application_identity::const_value::DecodedCanonicalConstValue;
    let declaration = program.const_declaration_by_symbol(symbol)?;
    let encoding = declaration.evaluated_value.as_ref()?.encoding()?;
    let decoded = CanonicalConstValue::new("", encoding, "").decode_encoding()?;
    let carrier = match decoded {
        DecodedCanonicalConstValue::Integer { integer_type, .. } => integer_type.name(),
        DecodedCanonicalConstValue::Boolean(_) => "bool".to_owned(),
        DecodedCanonicalConstValue::Array { .. }
        | DecodedCanonicalConstValue::Record { .. }
        | DecodedCanonicalConstValue::Variant { .. }
        | DecodedCanonicalConstValue::Float { .. }
        | DecodedCanonicalConstValue::StaticCallable { .. } => {
            program.normalized_type_identity_text(declaration.declared_type)
        }
    };
    Some(CanonicalConstValue::new(
        carrier,
        encoding,
        program.symbols.name(symbol),
    ))
}

/// Read a represented static value without evaluating source or proving its
/// carrier obligations. Anonymous integer landing remains the caller's work.
pub fn canonical_const_argument_value(
    program: &TypedTrees,
    reference: types::TypeReferenceHandle,
) -> Option<application_identity::const_value::CanonicalConstValue> {
    use application_identity::const_value::CanonicalConstValue;
    use application_identity::const_value::boolean_literal_spelling;
    if !program
        .type_reference_table
        .contains_type_reference(reference)
    {
        return None;
    }
    let value = match program.type_reference_table.type_reference(reference) {
        types::TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => {
            if !program.symbols.symbols().nodes().is_valid(*symbol) {
                return None;
            }
            canonical_const_declaration_value(program, *symbol)?
        }
        types::TypeReferenceNode::Named { name, .. } => {
            CanonicalConstValue::from_atom(name.as_str()).or_else(|| {
                boolean_literal_spelling(name.as_str()).map(CanonicalConstValue::boolean)
            })?
        }
        types::TypeReferenceNode::ConstExpression(expression) => {
            let expression::ExpressionNode::Boolean(value) =
                program.expression_table.expression(*expression)
            else {
                return None;
            };
            CanonicalConstValue::boolean(*value)
        }
        _ => return None,
    };
    value.decode_encoding()?;
    Some(value)
}

/// Whether this closed carrier has the copyable, address-free structural
/// shape represented by canonical index values. This proves no value predicate.
pub fn canonical_const_carrier_is_supported(
    program: &TypedTrees,
    reference: types::TypeReferenceHandle,
) -> bool {
    use data::DataMember;
    use types::TypeReferenceNode;
    let mut pending = vec![(reference, false)];
    let mut active = Vec::new();
    while let Some((reference, exiting)) = pending.pop() {
        if exiting {
            active.pop();
            continue;
        }
        if !program
            .type_reference_table
            .contains_type_reference(reference)
            || active.contains(&reference)
        {
            return false;
        }
        let node = program.type_reference_table.type_reference(reference);
        if let TypeReferenceNode::Named { symbol, .. } = node
            && program.symbols.symbols().nodes().is_valid(*symbol)
            && matches!(
                program.symbols.builtin_type_atom(*symbol),
                Some(symbols::BuiltinTypeAtom::Integer(_) | symbols::BuiltinTypeAtom::Bool)
            )
        {
            continue;
        }
        active.push(reference);
        pending.push((reference, true));
        if let TypeReferenceNode::FixedArray { element_type, .. } = node
            && program.closed_fixed_array_length(reference).is_some()
        {
            pending.push((*element_type, false));
            continue;
        }
        if !matches!(
            node,
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. }
        ) {
            return false;
        }
        let Some(data) = program.closed_data_definition(reference) else {
            return false;
        };
        if !data.lifetime_parameters.is_empty()
            || data.supply_mode != language_core::DataSupplyMode::CheckedShape
            || data.cleanup_selection.is_some()
            || data.quotient.is_some()
            || program.stored_value_multiplicity(reference)
                != language_core::Multiplicity::Unrestricted
        {
            return false;
        }
        for member in program.data_members(data) {
            match member {
                DataMember::Field(field) => pending.push((field.type_reference, false)),
                DataMember::Variant(variant) => pending.extend(
                    program
                        .data_payload_fields(variant)
                        .iter()
                        .map(|field| (field.type_reference, false)),
                ),
            }
        }
    }
    true
}

pub fn integer_const_declaration_extent(
    program: &TypedTrees,
    symbol: symbols::SymbolHandle,
) -> Option<usize> {
    let application_identity::const_value::DecodedCanonicalConstValue::Integer { value, .. } =
        canonical_const_declaration_value(program, symbol)?.decode_encoding()?
    else {
        return None;
    };
    usize::try_from(value).ok()
}

pub fn static_const_argument_spelling(
    program: &TypedTrees,
    argument: &expression::StaticMachineArgument,
) -> Option<String> {
    match static_const_argument_value(program, argument)? {
        StaticConstArgumentValue::Integer(value) => Some(value.to_string()),
        StaticConstArgumentValue::Literal(literal) => Some(literal.to_string()),
        StaticConstArgumentValue::Canonical(value) => canonical_static_const_spelling(value),
    }
}

pub(crate) enum StaticConstArgumentValue<'argument> {
    Integer(i128),
    Literal(&'argument numeric_literals::IntegerLiteral),
    Canonical(application_identity::const_value::CanonicalConstValue),
}

pub(crate) fn static_const_argument_value<'argument>(
    program: &TypedTrees,
    argument: &'argument expression::StaticMachineArgument,
) -> Option<StaticConstArgumentValue<'argument>> {
    use application_identity::const_value::CanonicalConstValue;
    use application_identity::const_value::boolean_literal_spelling;

    if argument.type_reference.is_valid()
        || argument.application.is_some()
        || argument.evidence_projection.is_some()
    {
        return None;
    }
    if let Some(literal) = &argument.const_literal {
        return Some(
            literal
                .value_i64()
                .map(i128::from)
                .or_else(|| literal.value_u64().map(i128::from))
                .or_else(|| literal.decimal_value_i128())
                .map_or(
                    StaticConstArgumentValue::Literal(literal),
                    StaticConstArgumentValue::Integer,
                ),
        );
    }
    if argument.symbol.is_valid() {
        let value = canonical_const_declaration_value(program, argument.symbol)?;
        return Some(StaticConstArgumentValue::Canonical(value));
    }
    let [name] = argument.path.as_ref() else {
        return None;
    };
    if let Some(value) = boolean_literal_spelling(name.as_str()) {
        return Some(StaticConstArgumentValue::Canonical(
            CanonicalConstValue::boolean(value),
        ));
    }
    CanonicalConstValue::from_atom(name.as_str()).map(StaticConstArgumentValue::Canonical)
}

pub(crate) fn canonical_static_const_spelling(
    value: application_identity::const_value::CanonicalConstValue,
) -> Option<String> {
    use application_identity::const_value::CanonicalConstValue;
    use application_identity::const_value::DecodedCanonicalConstValue;
    Some(match value.decode_encoding()? {
        DecodedCanonicalConstValue::Integer { value, .. } => value.to_string(),
        DecodedCanonicalConstValue::Boolean(value) => CanonicalConstValue::boolean(value).atom(),
        _ => value.atom(),
    })
}

//! Exact replay for the first seeded scalar-const instance rung.

use psi_language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use psi_symbol_resolved_trees::{
    SymbolResolvedTrees,
    data::{TypeParameter, TypeParameterKind},
    types::TypeReference,
};
use psi_symbols::{BuiltinTypeAtom, SymbolHandle, SymbolKind};

#[derive(Clone, Copy)]
enum ScalarConstCarrier {
    Integer(BuiltinTypeAtom),
    Boolean,
}

pub(super) fn parameter_is_supported(
    source: &SymbolResolvedTrees,
    owner: SymbolHandle,
    parameter: &TypeParameter,
) -> bool {
    parameter.symbol.is_valid()
        && source.symbols.get(parameter.symbol).kind == SymbolKind::TypeParameter
        && source.symbols.get(parameter.symbol).parent == owner
        && source.symbols.name(parameter.symbol) == parameter.name.as_str()
        && match &parameter.kind {
            TypeParameterKind::Type => true,
            TypeParameterKind::Const { type_reference } => {
                parameter.bounds == psi_symbol_resolved_trees::data::DataProperties::default()
                    && scalar_carrier(source, type_reference).is_some()
            }
            TypeParameterKind::Machine { .. } | TypeParameterKind::Proposition { .. } => false,
        }
}

pub(super) fn closed_argument_is_supported(
    source: &SymbolResolvedTrees,
    parameter: &TypeParameter,
    argument: &TypeReference,
) -> bool {
    let TypeParameterKind::Const { type_reference } = &parameter.kind else {
        return false;
    };
    let Some(carrier) = scalar_carrier(source, type_reference) else {
        return false;
    };
    let TypeReference::Named { symbol, name } = argument else {
        return false;
    };
    if symbol.is_valid() {
        return false;
    }
    match carrier {
        ScalarConstCarrier::Integer(carrier) => {
            canonical_integer(name.as_str()).is_some_and(|value| integer_fits(carrier, value))
        }
        ScalarConstCarrier::Boolean => canonical_boolean(name.as_str()),
    }
}

pub(super) fn template_argument_is_supported(
    source: &SymbolResolvedTrees,
    owner: SymbolHandle,
    owner_parameters: &[TypeParameter],
    parameter: &TypeParameter,
    argument: &TypeReference,
) -> bool {
    if closed_argument_is_supported(source, parameter, argument) {
        return true;
    }
    let TypeParameterKind::Const {
        type_reference: required_carrier,
    } = &parameter.kind
    else {
        return false;
    };
    let TypeReference::Named { symbol, name } = argument else {
        return false;
    };
    symbol.is_valid()
        && source.symbols.get(*symbol).kind == SymbolKind::TypeParameter
        && source.symbols.get(*symbol).parent == owner
        && source.symbols.name(*symbol) == name.as_str()
        && owner_parameters.iter().any(|candidate| {
            candidate.symbol == *symbol
                && matches!(
                    &candidate.kind,
                    TypeParameterKind::Const { type_reference }
                        if type_reference == required_carrier
                )
        })
}

pub(super) fn substituted_array_length_matches(
    source: &SymbolResolvedTrees,
    substitutions: &[(SymbolHandle, &TypeReference)],
    template: &psi_symbol_resolved_trees::types::FixedArrayLength,
    instance: &psi_symbol_resolved_trees::types::FixedArrayLength,
) -> bool {
    use psi_symbol_resolved_trees::types::FixedArrayLength;
    if template == instance {
        return true;
    }
    let (
        FixedArrayLength::ConstParameter { symbol, name },
        FixedArrayLength::Literal(instance_length),
    ) = (template, instance)
    else {
        return false;
    };
    symbol.is_valid()
        && source.symbols.get(*symbol).kind == SymbolKind::TypeParameter
        && source.symbols.name(*symbol) == name.as_str()
        && substitutions
            .iter()
            .find(|(parameter, _)| parameter == symbol)
            .and_then(|(_, argument)| {
                let TypeReference::Named {
                    symbol: argument_symbol,
                    name,
                } = argument
                else {
                    return None;
                };
                (!argument_symbol.is_valid())
                    .then(|| canonical_integer(name.as_str()))
                    .flatten()
            })
            .and_then(|value| usize::try_from(value).ok())
            == Some(*instance_length)
}

pub(super) fn array_length_is_supported(
    source: &SymbolResolvedTrees,
    owner: SymbolHandle,
    owner_parameters: &[TypeParameter],
    length: &psi_symbol_resolved_trees::types::FixedArrayLength,
) -> bool {
    use psi_symbol_resolved_trees::types::FixedArrayLength;
    match length {
        FixedArrayLength::Literal(_) => true,
        FixedArrayLength::ConstParameter { symbol, name } => {
            symbol.is_valid()
                && source.symbols.get(*symbol).kind == SymbolKind::TypeParameter
                && source.symbols.get(*symbol).parent == owner
                && source.symbols.name(*symbol) == name.as_str()
                && owner_parameters.iter().any(|parameter| {
                    parameter.symbol == *symbol
                        && matches!(
                            &parameter.kind,
                            TypeParameterKind::Const { type_reference }
                                if integer_carrier(source, type_reference).is_some()
                        )
                        && parameter_is_supported(source, owner, parameter)
                })
        }
        FixedArrayLength::ConstCall { .. } => false,
    }
}

fn scalar_carrier(
    source: &SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> Option<ScalarConstCarrier> {
    let TypeReference::Named { symbol, name } = type_reference else {
        return None;
    };
    if !symbol.is_valid() || source.symbols.name(*symbol) != name.as_str() {
        return None;
    }
    match source.symbols.builtin_type_atom(*symbol)? {
        BuiltinTypeAtom::Bool => Some(ScalarConstCarrier::Boolean),
        carrier @ (BuiltinTypeAtom::I8
        | BuiltinTypeAtom::I16
        | BuiltinTypeAtom::I32
        | BuiltinTypeAtom::I64
        | BuiltinTypeAtom::U8
        | BuiltinTypeAtom::U16
        | BuiltinTypeAtom::U32
        | BuiltinTypeAtom::U64
        | BuiltinTypeAtom::Address) => Some(ScalarConstCarrier::Integer(carrier)),
        _ => None,
    }
}

fn integer_carrier(
    source: &SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> Option<BuiltinTypeAtom> {
    match scalar_carrier(source, type_reference) {
        Some(ScalarConstCarrier::Integer(carrier)) => Some(carrier),
        Some(ScalarConstCarrier::Boolean) | None => None,
    }
}

fn canonical_integer(spelling: &str) -> Option<i128> {
    let value = spelling.parse::<i128>().ok()?;
    (value.to_string() == spelling).then_some(value)
}

fn canonical_boolean(spelling: &str) -> bool {
    let Some(value) = CanonicalConstValue::from_atom(spelling) else {
        return false;
    };
    let Some(DecodedCanonicalConstValue::Boolean(decoded)) = value.decode_encoding() else {
        return false;
    };
    value == CanonicalConstValue::boolean(decoded)
}

fn integer_fits(carrier: BuiltinTypeAtom, value: i128) -> bool {
    match carrier {
        BuiltinTypeAtom::I8 => i8::try_from(value).is_ok(),
        BuiltinTypeAtom::I16 => i16::try_from(value).is_ok(),
        BuiltinTypeAtom::I32 => i32::try_from(value).is_ok(),
        BuiltinTypeAtom::I64 => i64::try_from(value).is_ok(),
        BuiltinTypeAtom::U8 => u8::try_from(value).is_ok(),
        BuiltinTypeAtom::U16 => u16::try_from(value).is_ok(),
        BuiltinTypeAtom::U32 => u32::try_from(value).is_ok(),
        BuiltinTypeAtom::U64 | BuiltinTypeAtom::Address => u64::try_from(value).is_ok(),
        _ => false,
    }
}

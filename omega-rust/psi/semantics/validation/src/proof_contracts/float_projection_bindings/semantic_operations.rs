//! Source-operator bindings for the closed float-semantics catalog.
//!
//! The sealed toolchain source declares each representation-free semantic
//! definition as a bare bodyless `machine FloatSemantics::<name>(...)`.
//! Symbol resolution admits that shape only from the sealed file and only
//! when the authored spelling selects one catalog row; this pass re-checks the
//! typed shape by exact toolchain identity, so a sealed declaration whose
//! `FloatMeaning`, `FloatFormat` or `FloatClass` is not the toolchain-owned
//! type, or whose signature no longer matches its row, rejects instead of
//! surviving as an ordinary unsupplied declaration.

use diagnostics::Diagnostic;
use numerics::float_projection::FLOAT_PROJECTION_CORE_SOURCE;
use numerics::float_semantics_catalog::{
    FLOAT_FORMAT_CORE_SOURCE, FLOAT_SEMANTICS_NAMESPACE, FloatSemanticContractIdentity,
    FloatSemanticOperation, FloatSemanticValueKind, IntegerCarrier,
};
use typed_trees::TypedTrees;
use typed_trees::operator::OperatorDefinition;
use typed_trees::types::PrimitiveType;

pub(crate) fn symbol_is_declared_in_sealed_source(
    program: &TypedTrees,
    symbol: symbols::SymbolHandle,
    relative_source: &str,
) -> bool {
    let Some(span) = program.symbols.symbol_source_span(symbol) else {
        return false;
    };
    let Some(source) = program.symbols.source_file(span) else {
        return false;
    };
    source.origin == source::SourceOrigin::Toolchain
        && source
            .path
            .strip_prefix(&source.package_root)
            .ok()
            .is_some_and(|relative| relative == std::path::Path::new(relative_source))
}

fn has_exact_toolchain_float_semantics_owner(
    program: &TypedTrees,
    symbol: symbols::SymbolHandle,
    name: &str,
) -> bool {
    let expected_identity = format!("toolchain::{FLOAT_SEMANTICS_NAMESPACE}::{name}");
    program
        .normalized_hermetic_symbol_identity(symbol)
        .ok()
        .as_deref()
        == Some(expected_identity.as_str())
        && symbol_is_declared_in_sealed_source(program, symbol, FLOAT_PROJECTION_CORE_SOURCE)
}

/// Whether the type reference names the toolchain-owned data `name` declared
/// in the sealed `relative_source`, by exact hermetic identity and custody.
fn is_exact_toolchain_float_data_type(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    name: &str,
    relative_source: &str,
) -> bool {
    let typed_trees::types::TypeReferenceNode::Named {
        symbol,
        name: spelled,
    } = program.type_reference_table.type_reference(type_reference)
    else {
        return false;
    };
    spelled.as_str() == name
        && program
            .normalized_hermetic_symbol_identity(*symbol)
            .ok()
            .as_deref()
            == Some(format!("toolchain::{name}").as_str())
        && symbol_is_declared_in_sealed_source(program, *symbol, relative_source)
        && program
            .data_definitions()
            .iter()
            .any(|data| data.symbol == *symbol && data.name.as_str() == name)
}

/// Classify one typed position of a sealed semantic declaration by exact
/// toolchain identity (never by spelling).
fn typed_value_kind(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<FloatSemanticValueKind> {
    if let Some(primitive) = program.primitive_type_reference(type_reference) {
        return Some(match primitive {
            PrimitiveType::Bool => FloatSemanticValueKind::Bool,
            PrimitiveType::I8 => FloatSemanticValueKind::Integer(IntegerCarrier::I8),
            PrimitiveType::I16 => FloatSemanticValueKind::Integer(IntegerCarrier::I16),
            PrimitiveType::I32 => FloatSemanticValueKind::Integer(IntegerCarrier::I32),
            PrimitiveType::I64 => FloatSemanticValueKind::Integer(IntegerCarrier::I64),
            PrimitiveType::U8 => FloatSemanticValueKind::Integer(IntegerCarrier::U8),
            PrimitiveType::U16 => FloatSemanticValueKind::Integer(IntegerCarrier::U16),
            PrimitiveType::U32 => FloatSemanticValueKind::Integer(IntegerCarrier::U32),
            PrimitiveType::U64 => FloatSemanticValueKind::Integer(IntegerCarrier::U64),
            PrimitiveType::F32 | PrimitiveType::F64 | PrimitiveType::Addr => return None,
        });
    }
    if super::is_exact_toolchain_float_meaning_type(program, type_reference) {
        return Some(FloatSemanticValueKind::Meaning);
    }
    if is_exact_toolchain_float_data_type(
        program,
        type_reference,
        "FloatFormat",
        FLOAT_FORMAT_CORE_SOURCE,
    ) {
        return Some(FloatSemanticValueKind::Format);
    }
    if is_exact_toolchain_float_data_type(
        program,
        type_reference,
        "FloatClass",
        FLOAT_PROJECTION_CORE_SOURCE,
    ) {
        return Some(FloatSemanticValueKind::Class);
    }
    None
}

/// Recognize one complete sealed float-semantics declaration and return the
/// catalog row it selects with the row's contract identity. Toolchain/file
/// ownership, the public ordinary tokenless shape, and the exact toolchain
/// identity of every parameter and result type all bind as one closed row.
pub fn exact_toolchain_float_semantic_contract(
    program: &TypedTrees,
    operator: &OperatorDefinition,
) -> Option<(
    &'static FloatSemanticOperation,
    FloatSemanticContractIdentity,
)> {
    let [namespace, name] = program.operator_path_members(operator.name) else {
        return None;
    };
    if !FloatSemanticOperation::namespace_matches(namespace.as_str())
        || !has_exact_toolchain_float_semantics_owner(program, operator.symbol, name.as_str())
    {
        return None;
    }
    if !operator.is_public
        || operator.is_boundary
        || operator.spelling.is_some()
        || !operator.lifetime_parameters.is_empty()
        || !program.operator_type_parameters(operator).is_empty()
        || !program.operator_contracts(operator).is_empty()
    {
        return None;
    }
    let mut parameters = Vec::new();
    for parameter in program.operator_parameters(operator) {
        if parameter.is_const || parameter.is_mutable || parameter.is_self {
            return None;
        }
        parameters.push(typed_value_kind(program, parameter.type_reference)?);
    }
    let result = typed_value_kind(program, operator.return_type)?;
    let row = FloatSemanticOperation::from_source_identity(
        namespace.as_str(),
        name.as_str(),
        &parameters,
        result,
    )?;
    Some((row, row.contract_identity()))
}

pub(crate) fn validate_float_semantic_operation_bindings(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for operator in program.operators() {
        let [namespace, name] = program.operator_path_members(operator.name) else {
            continue;
        };
        if !FloatSemanticOperation::names_a_row(namespace.as_str(), name.as_str())
            || !has_exact_toolchain_float_semantics_owner(program, operator.symbol, name.as_str())
        {
            continue;
        }
        if exact_toolchain_float_semantic_contract(program, operator).is_none() {
            diagnostics.push(Diagnostic::error(format!(
                "canonical float semantic definition `{FLOAT_SEMANTICS_NAMESPACE}::{}` from `{FLOAT_PROJECTION_CORE_SOURCE}` must be one public, contract-free ordinary tokenless declaration whose parameters and result are exactly one catalog row over the sealed toolchain `FloatFormat`, `FloatMeaning`, `FloatClass`, `bool` and integer carriers, with no lifetime/type parameters or receiver qualification",
                name.as_str()
            )));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TypedTrees, validate_float_semantic_operation_bindings};
    use typed_trees::name::Identifier;
    use typed_trees::operator::OperatorDefinition;
    use typed_trees::signature::StateParameter;
    use typed_trees::types::TypeReferenceNode;

    fn semantic_lookalike_program() -> TypedTrees {
        let mut program = TypedTrees::default();
        let named = |program: &mut TypedTrees, name: &str| {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: Default::default(),
                    name: Identifier::generated(name),
                })
        };
        let format = named(&mut program, "FloatFormat");
        let meaning = named(&mut program, "FloatMeaning");
        let mut operator = OperatorDefinition {
            is_public: true,
            return_type: meaning,
            ..OperatorDefinition::default()
        };
        program.push_operator_path_member(&mut operator, Identifier::generated("FloatSemantics"));
        program.push_operator_path_member(&mut operator, Identifier::generated("add"));
        for (name, type_reference) in [("format", format), ("left", meaning), ("right", meaning)] {
            program.push_operator_parameter(
                &mut operator,
                StateParameter {
                    name: Identifier::generated(name),
                    type_reference,
                    ..StateParameter::default()
                },
            );
        }
        program.push_operator(operator);
        program
    }

    #[test]
    fn unowned_semantic_lookalike_remains_ordinary() {
        // Without toolchain custody the declaration is not the sealed row, so
        // this pass neither grants nor rejects it; symbol resolution already
        // refused the bare-signature form outside the sealed source.
        let mut diagnostics = Vec::new();
        validate_float_semantic_operation_bindings(&semantic_lookalike_program(), &mut diagnostics);
        assert!(diagnostics.is_empty());
    }
}

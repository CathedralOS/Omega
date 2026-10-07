//! Source-operator bindings for the closed float-projection catalog.
//!
//! The public proof projection is selected by its complete source identity and
//! exact signature. A similarly named operator cannot acquire projection
//! semantics, and a drifted canonical declaration fails before facts are built.

mod conversion_contracts;
/// The sibling closed catalog: `FloatSemantics::<name>` semantic definitions,
/// validated by the same custody rule. Its row/contract query
/// (`semantic_operations::exact_toolchain_float_semantic_contract`) is shared
/// by closed source evaluation and checked application binding. Neither source
/// query replaces the artifact verifier's independent catalog reconstruction.
pub(crate) mod semantic_operations;
pub(crate) mod semantic_values;
pub use conversion_contracts::{
    float_format_conversion_contract,
    float_integer_conversion_bounds, float_integer_conversion_contract,
    numeric_float_conversion_contract,
};

use diagnostics::Diagnostic;
use numerics::float_projection::{FloatProjectionContractIdentity, FloatProjectionOperation};
use numerics::float_semantics::FloatFormat;
use typed_trees::TypedTrees;
use typed_trees::operator::OperatorDefinition;
use typed_trees::types::PrimitiveType;

fn has_exact_toolchain_float_projection_owner(
    program: &TypedTrees,
    symbol: symbols::SymbolHandle,
    projection: FloatProjectionOperation,
) -> bool {
    let expected_identity = format!(
        "toolchain::{}::{}",
        projection.source_namespace(),
        projection.source_name(),
    );
    if program
        .normalized_hermetic_symbol_identity(symbol)
        .ok()
        .as_deref()
        != Some(expected_identity.as_str())
    {
        return false;
    }
    symbol_in_toolchain_source(
        program,
        symbol,
        std::path::Path::new(numerics::float_projection::FLOAT_PROJECTION_CORE_SOURCE),
    )
}

/// Recognize the sealed proof-only `FloatMeaning` carrier by exact toolchain
/// identity and source ownership. Callers must not grant D40 semantics from a
/// local same-named data declaration.
pub fn is_exact_toolchain_float_meaning_type(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    let typed_trees::types::TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return false;
    };
    if name.as_str() != "FloatMeaning"
        || program
            .normalized_hermetic_symbol_identity(*symbol)
            .ok()
            .as_deref()
            != Some("toolchain::FloatMeaning")
    {
        return false;
    }
    symbol_in_toolchain_source(
        program,
        *symbol,
        std::path::Path::new(numerics::float_projection::FLOAT_MEANING_CORE_SOURCE),
    ) && program
        .data_definitions()
        .iter()
        .any(|data| data.symbol == *symbol && data.name.as_str() == "FloatMeaning")
}

fn has_exact_toolchain_float_meaning_result(
    program: &TypedTrees,
    operator: &OperatorDefinition,
) -> bool {
    is_exact_toolchain_float_meaning_type(program, operator.return_type)
}

/// Recognize one complete sealed float-projection declaration and return its
/// exact source carrier. Operator spelling, toolchain/file ownership, ordinary
/// signature shape, and the toolchain-owned `FloatMeaning` result all bind as
/// one closed catalog row.
pub fn exact_toolchain_float_projection_primitive(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    projection: FloatProjectionOperation,
) -> Option<PrimitiveType> {
    exact_toolchain_float_projection_contract(program, operator, projection)
        .map(|(primitive, _)| primitive)
}

/// Recognize the same sealed declaration while retaining the exact rooted-
/// checker contract/catalog tuple which must survive into the artifact.
pub fn exact_toolchain_float_projection_contract(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    projection: FloatProjectionOperation,
) -> Option<(PrimitiveType, FloatProjectionContractIdentity)> {
    if !has_exact_toolchain_float_projection_owner(program, operator.symbol, projection) {
        return None;
    }
    let expected_primitive = if projection.rule().source_format == FloatFormat::BINARY32 {
        PrimitiveType::F32
    } else {
        debug_assert_eq!(projection.rule().source_format, FloatFormat::BINARY64);
        PrimitiveType::F64
    };
    let [parameter] = program.operator_parameters(operator) else {
        return None;
    };
    (!operator.is_public
        && !operator.is_boundary
        && operator.spelling.is_none()
        && operator.lifetime_parameters.is_empty()
        && program.operator_type_parameters(operator).is_empty()
        && !parameter.is_const
        && !parameter.is_mutable
        && program.operator_contracts(operator).is_empty()
        && program.primitive_type_reference(parameter.type_reference) == Some(expected_primitive)
        && has_exact_toolchain_float_meaning_result(program, operator))
    .then_some((expected_primitive, projection.contract_identity()))
}

pub(crate) fn validate_float_projection_operator_bindings(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    semantic_operations::validate_float_semantic_operation_bindings(program, diagnostics);
    for operator in program.operators() {
        let path = program.operator_path_members(operator.name);
        let [namespace, name] = path else {
            continue;
        };
        let Some(projection) =
            FloatProjectionOperation::from_source_identity(namespace.as_str(), name.as_str())
        else {
            continue;
        };

        let expected_primitive = if projection.rule().source_format == FloatFormat::BINARY32 {
            PrimitiveType::F32
        } else {
            debug_assert_eq!(projection.rule().source_format, FloatFormat::BINARY64);
            PrimitiveType::F64
        };
        if !has_exact_toolchain_float_projection_owner(program, operator.symbol, projection) {
            continue;
        }
        let signature_matches =
            exact_toolchain_float_projection_primitive(program, operator, projection)
                == Some(expected_primitive);
        if !signature_matches {
            diagnostics.push(Diagnostic::error(format!(
                "canonical float projection `{}::{}` from `{}` must be one private, contract-free ordinary tokenless operator from `{}` to the sealed toolchain `FloatMeaning` from `{}`, with no lifetime/type parameters or receiver qualification",
                projection.source_namespace(),
                projection.source_name(),
                numerics::float_projection::FLOAT_PROJECTION_CORE_SOURCE,
                match expected_primitive {
                    PrimitiveType::F32 => "f32",
                    PrimitiveType::F64 => "f64",
                    _ => unreachable!("closed float projection primitive"),
                },
                numerics::float_projection::FLOAT_MEANING_CORE_SOURCE,
            )));
        }
    }
}

/// `symbol` resolves to a declaration inside a toolchain-owned source file at
/// `relative` — the source-ownership half of the sealed-type recognizers.
fn symbol_in_toolchain_source(
    program: &TypedTrees,
    symbol: symbols::SymbolHandle,
    relative: &std::path::Path,
) -> bool {
    relative
        .to_str()
        .is_some_and(|relative| {
            semantic_operations::symbol_is_declared_in_sealed_source(program, symbol, relative)
        })
}

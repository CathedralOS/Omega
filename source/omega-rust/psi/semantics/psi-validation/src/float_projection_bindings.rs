//! Source-operator bindings for the closed float-projection catalog.
//!
//! The public proof projection is selected by its complete source identity and
//! exact signature. A similarly named operator cannot acquire projection
//! semantics, and a drifted canonical declaration fails before facts are built.

use psi_diagnostics::Diagnostic;
use psi_numerics::float_projection::FloatProjectionOperation;
use psi_numerics::float_semantics::FloatFormat;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::operator::OperatorDefinition;
use psi_typed_trees::types::PrimitiveType;

fn has_exact_toolchain_float_projection_owner(
    program: &TypedTrees,
    symbol: psi_symbols::SymbolHandle,
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
    let Some(span) = program.symbols.symbol_source_span(symbol) else {
        return false;
    };
    let Some(source) = program.symbols.source_file(span) else {
        return false;
    };
    source.origin == psi_source::SourceOrigin::Toolchain
        && source
            .path
            .strip_prefix(&source.package_root)
            .ok()
            .is_some_and(|relative| {
                relative
                    == std::path::Path::new(
                        psi_numerics::float_projection::FLOAT_PROJECTION_CORE_SOURCE,
                    )
            })
}

fn has_exact_toolchain_float_meaning_result(
    program: &TypedTrees,
    operator: &OperatorDefinition,
) -> bool {
    let psi_typed_trees::types::TypeReferenceNode::Named { symbol, name } = program
        .type_reference_table
        .type_reference(operator.return_type)
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
    let Some(span) = program.symbols.symbol_source_span(*symbol) else {
        return false;
    };
    let Some(source) = program.symbols.source_file(span) else {
        return false;
    };
    source.origin == psi_source::SourceOrigin::Toolchain
        && source
            .path
            .strip_prefix(&source.package_root)
            .ok()
            .is_some_and(|relative| {
                relative
                    == std::path::Path::new(
                        psi_numerics::float_projection::FLOAT_MEANING_CORE_SOURCE,
                    )
            })
        && program
            .data_definitions()
            .iter()
            .any(|data| data.symbol == *symbol && data.name.as_str() == "FloatMeaning")
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
    (!operator.is_boundary
        && operator.spelling.is_none()
        && operator.lifetime_parameters.is_empty()
        && program.operator_type_parameters(operator).is_empty()
        && !parameter.is_const
        && !parameter.is_mutable
        && !parameter.is_self
        && program.primitive_type_reference(parameter.type_reference) == Some(expected_primitive)
        && has_exact_toolchain_float_meaning_result(program, operator))
    .then_some(expected_primitive)
}

pub(crate) fn validate_float_projection_operator_bindings(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
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
                "canonical float projection `{}::{}` from `{}` must be one ordinary tokenless operator from `{}` to the sealed toolchain `FloatMeaning` from `{}`, with no lifetime/type parameters or receiver qualification",
                projection.source_namespace(),
                projection.source_name(),
                psi_numerics::float_projection::FLOAT_PROJECTION_CORE_SOURCE,
                match expected_primitive {
                    PrimitiveType::F32 => "f32",
                    PrimitiveType::F64 => "f64",
                    _ => unreachable!("closed float projection primitive"),
                },
                psi_numerics::float_projection::FLOAT_MEANING_CORE_SOURCE,
            )));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_typed_trees::name::Identifier;
    use psi_typed_trees::operator::OperatorDefinition;
    use psi_typed_trees::signature::StateParameter;
    use psi_typed_trees::types::TypeReferenceNode;

    fn projection_program(source: &str) -> TypedTrees {
        let mut program = TypedTrees::default();
        let source_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: Default::default(),
                name: Identifier::generated(source),
            });
        let result_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: Default::default(),
                name: Identifier::generated("FloatMeaning"),
            });
        let mut operator = OperatorDefinition {
            return_type: result_type,
            ..OperatorDefinition::default()
        };
        program.push_operator_path_member(&mut operator, Identifier::generated("Float"));
        program.push_operator_path_member(&mut operator, Identifier::generated("meaning32"));
        program.push_operator_parameter(
            &mut operator,
            StateParameter {
                name: Identifier::generated("value"),
                type_reference: source_type,
                ..StateParameter::default()
            },
        );
        program.push_operator(operator);
        program
    }

    #[test]
    fn unused_local_projection_lookalike_remains_ordinary() {
        for source in ["f32", "f64"] {
            let mut diagnostics = Vec::new();
            validate_float_projection_operator_bindings(
                &projection_program(source),
                &mut diagnostics,
            );
            assert!(diagnostics.is_empty());
        }
    }
}

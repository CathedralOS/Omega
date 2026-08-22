//! Source-operator bindings for the closed float-projection catalog.
//!
//! The public proof projection is selected by its complete source identity and
//! exact signature. A similarly named operator cannot acquire projection
//! semantics, and a drifted canonical declaration fails before facts are built.

use psi_diagnostics::Diagnostic;
use psi_numerics::float_projection::FloatProjectionOperation;
use psi_numerics::float_semantics::FloatFormat;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::types::PrimitiveType;

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
        let parameters = program.operator_parameters(operator);
        let signature_matches = !operator.is_boundary
            && operator.spelling.is_none()
            && operator.lifetime_parameters.is_empty()
            && program.operator_type_parameters(operator).is_empty()
            && parameters.len() == 1
            && parameters.first().is_some_and(|parameter| {
                !parameter.is_const
                    && !parameter.is_mutable
                    && !parameter.is_self
                    && program.primitive_type_reference(parameter.type_reference)
                        == Some(expected_primitive)
            })
            && program
                .named_type_reference(operator.return_type)
                .is_some_and(|name| name.as_str() == "FloatMeaning");
        if !signature_matches {
            diagnostics.push(Diagnostic::error(format!(
                "canonical float projection `{}::{}` must be one ordinary tokenless operator from `{}` to `FloatMeaning`, with no lifetime/type parameters or receiver qualification",
                projection.source_namespace(),
                projection.source_name(),
                match expected_primitive {
                    PrimitiveType::F32 => "f32",
                    PrimitiveType::F64 => "f64",
                    _ => unreachable!("closed float projection primitive"),
                },
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
    fn exact_projection_signature_binds() {
        let mut diagnostics = Vec::new();
        validate_float_projection_operator_bindings(&projection_program("f32"), &mut diagnostics);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn canonical_projection_rejects_source_format_drift() {
        let mut diagnostics = Vec::new();
        validate_float_projection_operator_bindings(&projection_program("f64"), &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("from `f32`"));
    }
}

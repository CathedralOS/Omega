//! Erase validated source float-projection invocations into checked proof rows.

use psi_checked_trees::{
    CheckedFloatMeaningProjection, CheckedFloatProjectionInput, CheckedFloatProjectionInputId,
    CheckedProofOnlyValueType, CheckedProofValueDeclaration, CheckedProofValueId, ProofFacts,
};
use psi_diagnostics::Diagnostic;
use psi_numerics::float_projection::FloatProjectionOperation;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::ExpressionNode;
use psi_typed_trees::operator::{resolve_named_call, resolve_named_expression_call};
use psi_validation::ValidatedFloatMeaningProjectionInvocation;

fn replay_invocation(
    program: &TypedTrees,
    fact: ValidatedFloatMeaningProjectionInvocation,
) -> Result<(), Diagnostic> {
    let ExpressionNode::Call(call) = program.expression_table.expression(fact.invocation) else {
        return Err(Diagnostic::error(
            "validated float-meaning projection invocation is no longer a call",
        ));
    };
    let operator = resolve_named_expression_call(program, call)
        .or_else(|| {
            let ExpressionNode::Name(path) = program.expression_table.expression(call.receiver)
            else {
                return None;
            };
            let [namespace] = program.expression_table.name_path_members(path.members) else {
                return None;
            };
            if namespace.as_str() != "Float" {
                return None;
            }
            let static_receiver = [namespace.as_str()];
            resolve_named_call(
                program,
                call.target_symbol,
                Some(&static_receiver),
                call.target.as_str(),
                program
                    .expression_table
                    .expression_handles(call.arguments)
                    .len(),
                false,
            )
        })
        .ok_or_else(|| {
            Diagnostic::error(
                "validated float-meaning projection operator no longer resolves exactly",
            )
        })?;
    if operator.symbol != fact.selected_operator_symbol {
        return Err(Diagnostic::error(
            "validated float-meaning projection operator identity drifted before checked binding",
        ));
    }
    let [namespace, name] = program.operator_path_members(operator.name) else {
        return Err(Diagnostic::error(
            "validated float-meaning projection operator path is no longer exact",
        ));
    };
    let operation =
        FloatProjectionOperation::from_source_identity(namespace.as_str(), name.as_str())
            .ok_or_else(|| {
                Diagnostic::error(
                    "validated float-meaning projection operator identity is not canonical",
                )
            })?;
    if operation != fact.operation {
        return Err(Diagnostic::error(
            "validated float-meaning projection operation drifted before checked binding",
        ));
    }
    let [parameter] = program.operator_parameters(operator) else {
        return Err(Diagnostic::error(
            "validated float-meaning projection signature no longer has one source parameter",
        ));
    };
    if !operator.symbol.is_valid()
        || operator.is_boundary
        || operator.spelling.is_some()
        || !operator.lifetime_parameters.is_empty()
        || !program.operator_type_parameters(operator).is_empty()
        || parameter.is_const
        || parameter.is_mutable
        || parameter.is_self
    {
        return Err(Diagnostic::error(
            "validated float-meaning projection declaration shape drifted before checked binding",
        ));
    }
    let [source] = program.expression_table.expression_handles(call.arguments) else {
        return Err(Diagnostic::error(
            "validated float-meaning projection call no longer has one source operand",
        ));
    };
    if *source != fact.source
        || program.primitive_type_reference(parameter.type_reference) != Some(fact.source_primitive)
        || !program
            .named_type_reference(operator.return_type)
            .is_some_and(|name| name.as_str() == "FloatMeaning")
    {
        return Err(Diagnostic::error(
            "validated float-meaning projection source/result shape drifted before checked binding",
        ));
    }
    Ok(())
}

pub(crate) fn bind_float_meaning_projection_facts(
    program: &TypedTrees,
    proof: &mut ProofFacts,
    facts: &[ValidatedFloatMeaningProjectionInvocation],
) -> Result<(), Vec<Diagnostic>> {
    let mut projections = Vec::with_capacity(facts.len());
    for (index, fact) in facts.iter().copied().enumerate() {
        replay_invocation(program, fact).map_err(|diagnostic| vec![diagnostic])?;
        let id = u32::try_from(index).map_err(|_| {
            vec![Diagnostic::error(
                "float-meaning projection plan exceeds its dense identity space",
            )]
        })?;
        let projection = CheckedFloatMeaningProjection {
            result: CheckedProofValueDeclaration {
                id: CheckedProofValueId(id),
                value_type: CheckedProofOnlyValueType::FloatMeaning,
            },
            source: CheckedFloatProjectionInput {
                id: CheckedFloatProjectionInputId(id),
                primitive: fact.source_primitive,
            },
            operation: fact.operation,
        };
        projection.validate().map_err(|_| {
            vec![Diagnostic::error(
                "checked float-meaning projection failed exact format replay",
            )]
        })?;
        projections.push(projection);
    }
    proof.float_meaning_projections = projections;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_checked_trees::{CheckedFloatProjectionInputId, CheckedProofValueId};
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;

    fn typed_projection_program() -> TypedTrees {
        let source = r#"
            data FloatMeaning { }
            operator Float::meaning32(value: f32) -> FloatMeaning;
            operator Float::meaning64(value: f64) -> FloatMeaning;

            machine prove(value32: f32, value64: f64)
            requires
                Float::meaning32(value32) == Float::meaning32(value32);
                Float::meaning64(value64) == Float::meaning64(value64);
            { }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        lower_symbol_resolved_trees(&resolved).expect("type")
    }

    #[test]
    fn actual_checked_projection_invocations_bind_to_dense_source_free_rows() {
        let checked = crate::lower_typed_trees(typed_projection_program()).expect("checked");
        let projections = &checked.facts.proof.float_meaning_projections;
        assert_eq!(projections.len(), 4);
        assert_eq!(projections[0].result.id, CheckedProofValueId(0));
        assert_eq!(projections[0].source.id, CheckedFloatProjectionInputId(0));
        assert_eq!(
            projections[0].operation,
            FloatProjectionOperation::Meaning32
        );
        assert_eq!(projections[1].result.id, CheckedProofValueId(1));
        assert_eq!(projections[1].source.id, CheckedFloatProjectionInputId(1));
        assert_eq!(projections[2].result.id, CheckedProofValueId(2));
        assert_eq!(projections[2].source.id, CheckedFloatProjectionInputId(2));
        assert_eq!(
            projections[2].operation,
            FloatProjectionOperation::Meaning64
        );
        assert_eq!(projections[3].result.id, CheckedProofValueId(3));
    }

    #[test]
    fn checked_binding_rejects_source_identity_substitution() {
        let program = typed_projection_program();
        let mut validation =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(&program)
                .expect("validate");
        assert_eq!(validation.float_meaning_projection_invocations.len(), 4);
        validation.float_meaning_projection_invocations[0].source =
            validation.float_meaning_projection_invocations[1].invocation;
        let mut proof = ProofFacts::default();
        let diagnostics = bind_float_meaning_projection_facts(
            &program,
            &mut proof,
            &validation.float_meaning_projection_invocations,
        )
        .expect_err("substituted source identity must reject");
        assert!(
            diagnostics[0]
                .message
                .contains("source/result shape drifted")
        );
        assert!(proof.float_meaning_projections.is_empty());
    }

    #[test]
    fn checked_binding_rejects_cross_format_operation_tamper() {
        let program = typed_projection_program();
        let mut validation =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(&program)
                .expect("validate");
        validation.float_meaning_projection_invocations[0].operation =
            FloatProjectionOperation::Meaning64;
        let mut proof = ProofFacts::default();
        let diagnostics = bind_float_meaning_projection_facts(
            &program,
            &mut proof,
            &validation.float_meaning_projection_invocations,
        )
        .expect_err("cross-format operation must reject");
        assert!(diagnostics[0].message.contains("operation drifted"));
        assert!(proof.float_meaning_projections.is_empty());
    }
}

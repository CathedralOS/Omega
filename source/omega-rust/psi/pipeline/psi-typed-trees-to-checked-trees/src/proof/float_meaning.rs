//! Erase validated source float-projection invocations into checked proof rows.

use psi_checked_trees::{
    CheckedFloatMeaningEqualityProposition, CheckedFloatMeaningProjection,
    CheckedFloatMeaningProjectionOccurrence, CheckedFloatMeaningProjectionOccurrenceId,
    CheckedFloatProjectionInput, CheckedFloatProjectionInputId, CheckedProofOnlyValueType,
    CheckedProofPropositionId, CheckedProofValueDeclaration, CheckedProofValueId, ProofFacts,
};
use psi_diagnostics::Diagnostic;
use psi_numerics::float_projection::FloatProjectionOperation;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{BinaryOperator, ExpressionNode};
use psi_typed_trees::operator::{resolve_named_call, resolve_named_expression_call};
use psi_typed_trees::types::PrimitiveType;
use psi_validation::{
    ValidatedFloatMeaningEqualityProposition, ValidatedFloatMeaningProjectionInvocation,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckedFloatProjectionSourceKey {
    ResolvedSymbol(psi_symbols::SymbolHandle),
    Binary32Literal(u32),
    Binary64Literal(u64),
    /// Transitional exact typed-expression custody for source forms whose
    /// artifact-reconstructible Terminal coordinate has not landed yet.
    TypedExpression(psi_typed_trees::expression::ExpressionHandle),
}

fn projection_source_key(
    program: &TypedTrees,
    fact: ValidatedFloatMeaningProjectionInvocation,
) -> CheckedFloatProjectionSourceKey {
    match program.expression_table.expression(fact.source) {
        ExpressionNode::Name(path) if path.symbol.is_valid() => {
            CheckedFloatProjectionSourceKey::ResolvedSymbol(path.symbol)
        }
        ExpressionNode::Float(literal) => match fact.source_primitive {
            PrimitiveType::F32 => {
                CheckedFloatProjectionSourceKey::Binary32Literal(literal.f32_bits())
            }
            PrimitiveType::F64 => {
                CheckedFloatProjectionSourceKey::Binary64Literal(literal.landed_f64().to_bits())
            }
            _ => CheckedFloatProjectionSourceKey::TypedExpression(fact.source),
        },
        _ => CheckedFloatProjectionSourceKey::TypedExpression(fact.source),
    }
}

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
    equality_facts: &[ValidatedFloatMeaningEqualityProposition],
) -> Result<(), Vec<Diagnostic>> {
    let mut projections = Vec::with_capacity(facts.len());
    let mut source_keys = Vec::<CheckedFloatProjectionSourceKey>::new();
    let mut projection_keys = Vec::<(
        CheckedFloatProjectionInputId,
        PrimitiveType,
        FloatProjectionOperation,
    )>::new();
    let mut invocation_values = Vec::with_capacity(facts.len());
    let mut occurrences = Vec::with_capacity(facts.len());
    for (index, fact) in facts.iter().copied().enumerate() {
        replay_invocation(program, fact).map_err(|diagnostic| vec![diagnostic])?;
        let source_key = projection_source_key(program, fact);
        let source_index = match source_keys.iter().position(|key| *key == source_key) {
            Some(index) => index,
            None => {
                source_keys.push(source_key);
                source_keys.len() - 1
            }
        };
        let source_id =
            CheckedFloatProjectionInputId(u32::try_from(source_index).map_err(|_| {
                vec![Diagnostic::error(
                    "float-meaning projection sources exceed their dense identity space",
                )]
            })?);
        let projection_key = (source_id, fact.source_primitive, fact.operation);
        let value_index = match projection_keys
            .iter()
            .position(|key| *key == projection_key)
        {
            Some(index) => index,
            None => {
                projection_keys.push(projection_key);
                let id = u32::try_from(projections.len()).map_err(|_| {
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
                        id: source_id,
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
                projections.len() - 1
            }
        };
        let value = CheckedProofValueId(u32::try_from(value_index).map_err(|_| {
            vec![Diagnostic::error(
                "float-meaning projection plan exceeds its dense identity space",
            )]
        })?);
        invocation_values.push((fact.invocation, value));
        occurrences.push(CheckedFloatMeaningProjectionOccurrence {
            id: CheckedFloatMeaningProjectionOccurrenceId(u32::try_from(index).map_err(|_| {
                vec![Diagnostic::error(
                    "float-meaning projection occurrences exceed their dense identity space",
                )]
            })?),
            value,
            source_span: program.expression_table.source_span(fact.invocation),
        });
    }
    if projection_keys.len() != projections.len() {
        return Err(vec![Diagnostic::error(
            "checked float-meaning projection canonicalization lost a semantic key",
        )]);
    }
    let mut equalities = Vec::with_capacity(equality_facts.len());
    for (index, fact) in equality_facts.iter().copied().enumerate() {
        let ExpressionNode::Binary(expression) =
            program.expression_table.expression(fact.expression)
        else {
            return Err(vec![Diagnostic::error(
                "validated float-meaning equality is no longer a binary proposition",
            )]);
        };
        if expression.operator != BinaryOperator::Equal
            || expression.left != fact.left
            || expression.right != fact.right
        {
            return Err(vec![Diagnostic::error(
                "validated float-meaning equality identity drifted before checked binding",
            )]);
        }
        let left = invocation_values
            .iter()
            .find(|(invocation, _)| *invocation == fact.left)
            .map(|(_, value)| value.0)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "validated float-meaning equality lost its left projection",
                )]
            })?;
        let right = invocation_values
            .iter()
            .find(|(invocation, _)| *invocation == fact.right)
            .map(|(_, value)| value.0)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "validated float-meaning equality lost its right projection",
                )]
            })?;
        let id = u32::try_from(index).map_err(|_| {
            vec![Diagnostic::error(
                "float-meaning equality plan exceeds its dense identity space",
            )]
        })?;
        equalities.push(CheckedFloatMeaningEqualityProposition {
            id: CheckedProofPropositionId(id),
            left: CheckedProofValueId(left.min(right)),
            right: CheckedProofValueId(left.max(right)),
        });
    }
    proof.float_meaning_projections = projections;
    proof.float_meaning_projection_occurrences = occurrences;
    proof.float_meaning_equalities = equalities;
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
    fn actual_checked_projection_invocations_deduplicate_values_and_retain_occurrences() {
        let checked = crate::lower_typed_trees(typed_projection_program()).expect("checked");
        let projections = &checked.facts.proof.float_meaning_projections;
        assert_eq!(projections.len(), 2);
        assert_eq!(projections[0].result.id, CheckedProofValueId(0));
        assert_eq!(projections[0].source.id, CheckedFloatProjectionInputId(0));
        assert_eq!(
            projections[0].operation,
            FloatProjectionOperation::Meaning32
        );
        assert_eq!(projections[1].result.id, CheckedProofValueId(1));
        assert_eq!(projections[1].source.id, CheckedFloatProjectionInputId(1));
        assert_eq!(
            projections[1].operation,
            FloatProjectionOperation::Meaning64
        );
        let occurrences = &checked.facts.proof.float_meaning_projection_occurrences;
        assert_eq!(occurrences.len(), 4);
        assert_eq!(occurrences[0].value, CheckedProofValueId(0));
        assert_eq!(occurrences[1].value, CheckedProofValueId(0));
        assert_eq!(occurrences[2].value, CheckedProofValueId(1));
        assert_eq!(occurrences[3].value, CheckedProofValueId(1));
        assert_eq!(occurrences[0].id.0, 0);
        assert_eq!(occurrences[1].id.0, 1);
        assert_eq!(checked.facts.proof.float_meaning_equalities.len(), 2);
        assert_eq!(
            checked.facts.proof.float_meaning_equalities[0],
            CheckedFloatMeaningEqualityProposition {
                id: CheckedProofPropositionId(0),
                left: CheckedProofValueId(0),
                right: CheckedProofValueId(0),
            }
        );
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
            &validation.float_meaning_equality_propositions,
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
            &validation.float_meaning_equality_propositions,
        )
        .expect_err("cross-format operation must reject");
        assert!(diagnostics[0].message.contains("operation drifted"));
        assert!(proof.float_meaning_projections.is_empty());
    }

    #[test]
    fn checked_binding_rejects_equality_operand_substitution_transactionally() {
        let program = typed_projection_program();
        let mut validation =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(&program)
                .expect("validate");
        validation.float_meaning_equality_propositions[0].left =
            validation.float_meaning_projection_invocations[2].invocation;
        let mut proof = ProofFacts::default();
        let diagnostics = bind_float_meaning_projection_facts(
            &program,
            &mut proof,
            &validation.float_meaning_projection_invocations,
            &validation.float_meaning_equality_propositions,
        )
        .expect_err("substituted equality operand must reject");
        assert!(diagnostics[0].message.contains("identity drifted"));
        assert!(proof.float_meaning_projections.is_empty());
        assert!(proof.float_meaning_equalities.is_empty());
    }
}

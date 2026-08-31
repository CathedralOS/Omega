//! Erase validated source float-projection invocations into checked proof rows.

use psi_checked_trees::{
    CheckedFloatMeaningEqualityProposition, CheckedFloatMeaningProjection,
    CheckedFloatMeaningProjectionOccurrence, CheckedFloatMeaningProjectionOccurrenceId,
    CheckedFloatProjectionInput, CheckedFloatProjectionInputId, CheckedFloatProjectionSource,
    CheckedProofOnlyValueType, CheckedProofPropositionId, CheckedProofValueDeclaration,
    CheckedProofValueId, ProofFacts,
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
    let Some(replayed_primitive) =
        psi_validation::exact_toolchain_float_projection_primitive(program, operator, operation)
    else {
        return Err(Diagnostic::error(
            "validated float-meaning projection lost its sealed toolchain declaration before checked binding",
        ));
    };
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
    if *source != fact.source || replayed_primitive != fact.source_primitive {
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
    let mut transitional_source_keys = Vec::<CheckedFloatProjectionSourceKey>::new();
    let mut projection_keys =
        Vec::<(CheckedFloatProjectionSource, FloatProjectionOperation)>::new();
    let mut invocation_values = Vec::with_capacity(facts.len());
    let mut occurrences = Vec::with_capacity(facts.len());
    for (index, fact) in facts.iter().copied().enumerate() {
        replay_invocation(program, fact).map_err(|diagnostic| vec![diagnostic])?;
        let source_key = projection_source_key(program, fact);
        let source = match source_key {
            CheckedFloatProjectionSourceKey::Binary32Literal(bits) => {
                CheckedFloatProjectionSource::ExactBinary32Literal(bits)
            }
            CheckedFloatProjectionSourceKey::Binary64Literal(bits) => {
                CheckedFloatProjectionSource::ExactBinary64Literal(bits)
            }
            transitional => {
                let source_index = match transitional_source_keys
                    .iter()
                    .position(|key| *key == transitional)
                {
                    Some(index) => index,
                    None => {
                        transitional_source_keys.push(transitional);
                        transitional_source_keys.len() - 1
                    }
                };
                let source_id =
                    CheckedFloatProjectionInputId(u32::try_from(source_index).map_err(|_| {
                        vec![Diagnostic::error(
                            "float-meaning projection sources exceed their dense identity space",
                        )]
                    })?);
                CheckedFloatProjectionSource::TransitionalInput(CheckedFloatProjectionInput {
                    id: source_id,
                    primitive: fact.source_primitive,
                })
            }
        };
        let projection_key = (source, fact.operation);
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
                    source,
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
    use psi_source::{SourceMap, SourceOrigin};
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::{
        lower_syntax_trees, lower_syntax_trees_with_sources,
    };
    use psi_tokens_to_syntax_trees::{
        parse_syntax_trees, parse_syntax_trees_into_with_id, parse_syntax_trees_with_id,
    };
    use std::path::PathBuf;
    use std::sync::Arc;

    const CORE_FLOAT_MEANING: &str = "data FloatMeaning { }";
    const CORE_PROJECTIONS: &str = r#"
        operator Float::meaning32(value: f32) -> FloatMeaning;
        operator Float::meaning64(value: f64) -> FloatMeaning;
    "#;

    fn lower_projection_fixture(source: &str) -> TypedTrees {
        lower_projection_fixture_with_meaning_origin(source, SourceOrigin::Toolchain)
    }

    fn lower_projection_fixture_with_meaning_origin(
        source: &str,
        meaning_origin: SourceOrigin,
    ) -> TypedTrees {
        lower_projection_fixture_with_metadata(
            source,
            meaning_origin,
            CORE_PROJECTIONS,
            "float_operations.omg",
            SourceOrigin::Toolchain,
        )
    }

    fn lower_projection_fixture_with_metadata(
        source: &str,
        meaning_origin: SourceOrigin,
        projection_declarations: &str,
        projection_file: &str,
        projection_origin: SourceOrigin,
    ) -> TypedTrees {
        let mut sources = SourceMap::default();
        let meaning_source_id = sources
            .add_with_metadata(
                PathBuf::from("source/library/core/float_meaning.omg"),
                CORE_FLOAT_MEANING.to_owned(),
                PathBuf::from("source/library/core"),
                None,
                meaning_origin,
            )
            .source_id;
        let projection_source_id = sources
            .add_with_metadata(
                PathBuf::from("source/library/core").join(projection_file),
                projection_declarations.to_owned(),
                PathBuf::from("source/library/core"),
                None,
                projection_origin,
            )
            .source_id;
        let user_source_id = sources
            .add(
                PathBuf::from("tests/float_projection/main.omg"),
                source.to_owned(),
            )
            .source_id;
        let meaning_tokens = Lexer::new(CORE_FLOAT_MEANING)
            .tokenize()
            .expect("tokenize float meaning");
        let mut syntax = parse_syntax_trees_with_id(meaning_source_id, &meaning_tokens)
            .expect("parse float meaning");
        let projection_tokens = Lexer::new(projection_declarations)
            .tokenize()
            .expect("tokenize projections");
        parse_syntax_trees_into_with_id(&mut syntax, projection_source_id, &projection_tokens)
            .expect("parse core projections");
        let user_tokens = Lexer::new(source).tokenize().expect("tokenize fixture");
        parse_syntax_trees_into_with_id(&mut syntax, user_source_id, &user_tokens)
            .expect("parse fixture");
        let resolved = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
            .expect("resolve source-aware projection fixture");
        lower_symbol_resolved_trees(&resolved).expect("type projection fixture")
    }

    fn lower_local_projection_lookalike(source: &str) -> TypedTrees {
        let combined = format!("{CORE_FLOAT_MEANING}\n{CORE_PROJECTIONS}\n{source}");
        let tokens = Lexer::new(&combined)
            .tokenize()
            .expect("tokenize lookalike");
        let syntax = parse_syntax_trees(&tokens).expect("parse lookalike");
        let resolved = lower_syntax_trees(&syntax).expect("resolve lookalike");
        lower_symbol_resolved_trees(&resolved).expect("type lookalike")
    }

    fn typed_projection_program() -> TypedTrees {
        lower_projection_fixture(projection_source())
    }

    fn local_projection_program() -> TypedTrees {
        lower_local_projection_lookalike(projection_source())
    }

    fn projection_source() -> &'static str {
        r#"
            machine prove(value32: f32, value64: f64)
            requires
                Float::meaning32(value32) == Float::meaning32(value32);
                Float::meaning64(value64) == Float::meaning64(value64);
            { }
        "#
    }

    fn typed_literal_projection_program() -> TypedTrees {
        let source = r#"
            machine prove()
            requires
                Float::meaning32(0.0f32) == Float::meaning32(0.00f32);
                Float::meaning32(-0.0f32) == Float::meaning32(-0.00f32);
                Float::meaning64(0.1f64) == Float::meaning64(0.10f64);
            { }
        "#;
        lower_projection_fixture(source)
    }

    #[test]
    fn actual_checked_projection_invocations_deduplicate_values_and_retain_occurrences() {
        let checked = crate::lower_typed_trees(typed_projection_program()).expect("checked");
        let projections = &checked.facts.proof.float_meaning_projections;
        assert_eq!(projections.len(), 2);
        assert_eq!(projections[0].result.id, CheckedProofValueId(0));
        assert_eq!(
            projections[0].source,
            CheckedFloatProjectionSource::TransitionalInput(CheckedFloatProjectionInput {
                id: CheckedFloatProjectionInputId(0),
                primitive: PrimitiveType::F32,
            })
        );
        assert_eq!(
            projections[0].operation,
            FloatProjectionOperation::Meaning32
        );
        assert_eq!(projections[1].result.id, CheckedProofValueId(1));
        assert_eq!(
            projections[1].source,
            CheckedFloatProjectionSource::TransitionalInput(CheckedFloatProjectionInput {
                id: CheckedFloatProjectionInputId(1),
                primitive: PrimitiveType::F64,
            })
        );
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
    fn exact_literal_bits_are_the_checked_semantic_source_identity() {
        let checked =
            crate::lower_typed_trees(typed_literal_projection_program()).expect("checked");
        let projections = &checked.facts.proof.float_meaning_projections;
        assert_eq!(projections.len(), 3);
        assert_eq!(
            projections[0].source,
            CheckedFloatProjectionSource::ExactBinary32Literal(0.0_f32.to_bits())
        );
        assert_eq!(
            projections[1].source,
            CheckedFloatProjectionSource::ExactBinary32Literal((-0.0_f32).to_bits())
        );
        assert_eq!(
            projections[2].source,
            CheckedFloatProjectionSource::ExactBinary64Literal(0.1_f64.to_bits())
        );
        assert_ne!(projections[0].result.id, projections[1].result.id);
        assert_eq!(
            checked
                .facts
                .proof
                .float_meaning_projection_occurrences
                .iter()
                .map(|occurrence| occurrence.value)
                .collect::<Vec<_>>(),
            vec![
                CheckedProofValueId(0),
                CheckedProofValueId(0),
                CheckedProofValueId(1),
                CheckedProofValueId(1),
                CheckedProofValueId(2),
                CheckedProofValueId(2),
            ]
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
    fn proof_projection_call_rejects_a_local_operator_lookalike() {
        let diagnostics =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(
                &local_projection_program(),
            )
            .expect_err("a local projection spelling has no closed-catalog authority");
        assert!(
            diagnostics[0]
                .message
                .contains("did not resolve one exact canonical operator signature"),
            "unexpected diagnostic: {:?}",
            diagnostics[0],
        );
    }

    #[test]
    fn canonical_projection_rejects_a_user_owned_float_meaning_result() {
        let program =
            lower_projection_fixture_with_meaning_origin(projection_source(), SourceOrigin::User);
        let diagnostics =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(&program)
                .expect_err("the canonical operator cannot return a user FloatMeaning lookalike");
        assert!(
            diagnostics[0]
                .message
                .contains("sealed toolchain `FloatMeaning`"),
            "unexpected diagnostic: {:?}",
            diagnostics[0],
        );
    }

    #[test]
    fn projection_call_rejects_a_toolchain_declaration_from_the_wrong_file() {
        let program = lower_projection_fixture_with_metadata(
            projection_source(),
            SourceOrigin::Toolchain,
            CORE_PROJECTIONS,
            "float_projection_lookalike.omg",
            SourceOrigin::Toolchain,
        );
        let diagnostics =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(&program)
                .expect_err("a different toolchain file cannot own Float projection semantics");
        assert!(
            diagnostics[0]
                .message
                .contains("did not resolve one exact canonical operator signature"),
            "unexpected diagnostic: {:?}",
            diagnostics[0],
        );
    }

    #[test]
    fn canonical_projection_declaration_rejects_source_format_drift() {
        let program = lower_projection_fixture_with_metadata(
            "machine main() { }",
            SourceOrigin::Toolchain,
            r#"
                operator Float::meaning32(value: f64) -> FloatMeaning;
                operator Float::meaning64(value: f64) -> FloatMeaning;
            "#,
            "float_operations.omg",
            SourceOrigin::Toolchain,
        );
        let diagnostics =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(&program)
                .expect_err("the sealed meaning32 declaration cannot drift to binary64");
        assert!(
            diagnostics[0].message.contains("from `f32`"),
            "unexpected diagnostic: {:?}",
            diagnostics[0],
        );
    }

    #[test]
    fn checked_binding_rejects_validated_facts_replayed_on_a_local_lookalike() {
        let canonical = typed_projection_program();
        let validation =
            psi_validation::validate_program_after_generic_contract_entailment_with_facts(
                &canonical,
            )
            .expect("validate canonical toolchain projections");
        let local = local_projection_program();
        let mut proof = ProofFacts::default();
        let diagnostics = bind_float_meaning_projection_facts(
            &local,
            &mut proof,
            &validation.float_meaning_projection_invocations,
            &validation.float_meaning_equality_propositions,
        )
        .expect_err("validated facts cannot transfer to a local projection lookalike");
        assert!(
            diagnostics[0]
                .message
                .contains("sealed toolchain declaration"),
            "unexpected diagnostic: {:?}",
            diagnostics[0],
        );
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

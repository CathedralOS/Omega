//! Float meaning projection tests.

use super::{
    BinaryOperator, CheckedFloatProjectionInput, CheckedFloatProjectionSource,
    CheckedProofPropositionId, ExpressionNode, FloatProjectionOperation, PrimitiveType, ProofFacts,
    TypedTrees,
};
use crate::proof::bind_float_meaning_projection_facts;
use checked_trees::{CheckedFloatProjectionInputId, CheckedProofValueId};
use source::{SourceMap, SourceOrigin};
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::{
    parse_syntax_trees, parse_syntax_trees_into_with_id, parse_syntax_trees_with_id,
};

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
    let resolved = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve source-aware projection fixture");
    lower_symbol_resolved_trees(&resolved).expect("type projection fixture")
}

fn lower_local_projection_lookalike(source: &str) -> TypedTrees {
    let combined = format!("{CORE_FLOAT_MEANING}\n{CORE_PROJECTIONS}\n{source}");
    let tokens = Lexer::new(&combined)
        .tokenize()
        .expect("tokenize lookalike");
    let syntax = parse_syntax_trees(&tokens).expect("parse lookalike");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve lookalike");
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

fn transitional_source(input: CheckedFloatProjectionInput) -> CheckedFloatProjectionSource {
    CheckedFloatProjectionSource::TransitionalInput(input)
}

fn bind_projection_facts_without_exit_proof(program: &TypedTrees) -> ProofFacts {
    let validation = validation::validate_specialized_program(
        program,
        validation::OpaquePropertyValidation::Required(&[]),
    )
    .map(|validated| validated.facts)
    .expect("validate projection facts");
    let proof_plan = proof::obligations::build_proof_plan(program);
    let borrow = crate::borrow::build_borrow_facts(program);
    let mut proof = crate::proof::build_proof_facts(program, &proof_plan, &borrow);
    bind_float_meaning_projection_facts(
        program,
        &mut proof,
        &validation.float_meaning_projection_invocations,
        &validation.float_meaning_equality_propositions,
    )
    .expect("bind projection facts");
    proof
}

#[test]
fn actual_checked_projection_invocations_deduplicate_values_and_retain_occurrences() {
    let checked = crate::lower_typed_trees(typed_projection_program()).expect("checked");
    let projections = &checked.facts.proof.float_meaning_projections;
    assert_eq!(projections.len(), 2);
    assert_eq!(projections[0].result.id, CheckedProofValueId(0));
    let CheckedFloatProjectionSource::DirectMachineParameter(narrow) = projections[0].source else {
        panic!("direct f32 parameter should retain checked provenance")
    };
    assert_eq!(checked.symbols.name(narrow.owner_machine), "prove");
    assert_eq!(checked.symbols.name(narrow.parameter), "value32");
    assert_eq!(
        narrow.fallback,
        CheckedFloatProjectionInput {
            id: CheckedFloatProjectionInputId(0),
            primitive: PrimitiveType::F32,
        }
    );
    assert_eq!(
        projections[0].operation,
        FloatProjectionOperation::Meaning32
    );
    assert_eq!(projections[1].result.id, CheckedProofValueId(1));
    let CheckedFloatProjectionSource::DirectMachineParameter(wide) = projections[1].source else {
        panic!("direct f64 parameter should retain checked provenance")
    };
    assert_eq!(wide.owner_machine, narrow.owner_machine);
    assert_ne!(wide.parameter, narrow.parameter);
    assert_eq!(checked.symbols.name(wide.parameter), "value64");
    assert_eq!(
        wide.fallback,
        CheckedFloatProjectionInput {
            id: CheckedFloatProjectionInputId(1),
            primitive: PrimitiveType::F64,
        }
    );
    assert_eq!(
        projections[1].operation,
        FloatProjectionOperation::Meaning64
    );
    let owner = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == narrow.owner_machine)
        .expect("retained owner machine");
    let entry = checked
        .machine_states(owner)
        .first()
        .expect("machine entry state");
    let parameters = checked.state_parameters(entry);
    assert_eq!(parameters[0].symbol, narrow.parameter);
    assert_eq!(parameters[1].symbol, wide.parameter);
    assert_eq!(
        checked.primitive_type_reference(parameters[0].type_reference),
        Some(PrimitiveType::F32)
    );
    assert_eq!(
        checked.primitive_type_reference(parameters[1].type_reference),
        Some(PrimitiveType::F64)
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
    let narrow_equality = checked.facts.proof.float_meaning_equalities[0];
    assert_eq!(narrow_equality.id, CheckedProofPropositionId(0));
    assert_eq!(narrow_equality.left, CheckedProofValueId(0));
    assert_eq!(narrow_equality.right, CheckedProofValueId(0));
    assert!(matches!(
        checked
            .expression_table
            .expression(narrow_equality.source_expression),
        ExpressionNode::Binary(expression) if expression.operator == BinaryOperator::Equal
    ));
}

#[test]
fn direct_parameter_identity_includes_its_exact_machine_owner() {
    let checked = crate::lower_typed_trees(lower_projection_fixture(
        r#"
                machine narrow(value: f32)
                requires Float::meaning32(value) == Float::meaning32(value);
                {}

                machine wide(value: f32)
                requires Float::meaning32(value) == Float::meaning32(value);
                {}
            "#,
    ))
    .expect("checked");
    let [narrow, wide] = checked.facts.proof.float_meaning_projections.as_slice() else {
        panic!("one projection for each exact machine parameter")
    };
    let CheckedFloatProjectionSource::DirectMachineParameter(narrow) = narrow.source else {
        panic!("narrow source should retain direct parameter provenance")
    };
    let CheckedFloatProjectionSource::DirectMachineParameter(wide) = wide.source else {
        panic!("wide source should retain direct parameter provenance")
    };
    assert_eq!(checked.symbols.name(narrow.owner_machine), "narrow");
    assert_eq!(checked.symbols.name(wide.owner_machine), "wide");
    assert_ne!(narrow.owner_machine, wide.owner_machine);
    assert_ne!(narrow.parameter, wide.parameter);
    assert_eq!(narrow.fallback.id, CheckedFloatProjectionInputId(0));
    assert_eq!(wide.fallback.id, CheckedFloatProjectionInputId(1));
}

#[test]
fn top_level_scalar_result_retains_direct_checked_provenance() {
    let checked = crate::lower_typed_trees(lower_projection_fixture(
        r#"
                machine result_source(value: f32) -> f32
                ensures Float::meaning32(result) == Float::meaning32(result);
                { value }
            "#,
    ))
    .expect("direct result reflexivity should pass ordinary exit checking");
    let result_proof = &checked.facts.proof;
    let CheckedFloatProjectionSource::DirectMachineResult(result) =
        result_proof.float_meaning_projections[0].source
    else {
        panic!("top-level scalar result should retain direct provenance")
    };
    assert_eq!(checked.symbols.name(result.owner_machine), "result_source");
    assert_eq!(
        result.fallback,
        CheckedFloatProjectionInput {
            id: CheckedFloatProjectionInputId(0),
            primitive: PrimitiveType::F32,
        }
    );
}

#[test]
fn direct_result_identity_includes_exact_owner_and_primitive_format() {
    let checked = crate::lower_typed_trees(lower_projection_fixture(
        r#"
                machine narrow(value: f32) -> f32
                ensures Float::meaning32(result) == Float::meaning32(result);
                { value }

                machine wide(value: f64) -> f64
                ensures Float::meaning64(result) == Float::meaning64(result);
                { value }
            "#,
    ))
    .expect("direct result reflexivity should pass for both primitive formats");
    let proof = &checked.facts.proof;
    let [narrow, wide] = proof.float_meaning_projections.as_slice() else {
        panic!("one result projection per owning machine")
    };
    let CheckedFloatProjectionSource::DirectMachineResult(narrow) = narrow.source else {
        panic!("narrow result provenance")
    };
    let CheckedFloatProjectionSource::DirectMachineResult(wide) = wide.source else {
        panic!("wide result provenance")
    };
    assert_eq!(checked.symbols.name(narrow.owner_machine), "narrow");
    assert_eq!(checked.symbols.name(wide.owner_machine), "wide");
    assert_ne!(narrow.owner_machine, wide.owner_machine);
    assert_eq!(narrow.fallback.primitive, PrimitiveType::F32);
    assert_eq!(wide.fallback.primitive, PrimitiveType::F64);
}

#[test]
fn direct_result_reflexivity_does_not_prove_a_distinct_parameter_projection() {
    let diagnostics = crate::lower_typed_trees(lower_projection_fixture(
        r#"
                machine distinct(value: f32) -> f32
                ensures Float::meaning32(result) == Float::meaning32(value);
                { value }
            "#,
    ))
    .expect_err("distinct checked projection terms require explicit evidence");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove ensures contract for exit from distinct")
    }));
}

#[test]
fn raw_float_result_equality_does_not_borrow_float_meaning_reflexivity() {
    let diagnostics = crate::lower_typed_trees(lower_projection_fixture(
        r#"
                machine raw(value: f32) -> f32
                ensures result == result;
                { value }
            "#,
    ))
    .expect_err("IEEE equality is not FloatMeaning structural equality");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove ensures contract for exit from raw")
    }));
}

#[test]
fn real_result_named_parameter_shadows_the_contract_pseudo_result() {
    let program = lower_projection_fixture(
        r#"
                machine shadow(result: f32) -> f32
                ensures Float::meaning32(result) == Float::meaning32(result);
                { result }
            "#,
    );
    let proof = bind_projection_facts_without_exit_proof(&program);
    let CheckedFloatProjectionSource::DirectMachineParameter(parameter) =
        proof.float_meaning_projections[0].source
    else {
        panic!("real parameter must shadow the reserved pseudo-result")
    };
    assert_eq!(program.symbols.name(parameter.owner_machine), "shadow");
    assert_eq!(program.symbols.name(parameter.parameter), "result");
    let diagnostics = crate::lower_typed_trees(program)
        .expect_err("a real result parameter must not receive pseudo-result reflexivity");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove ensures contract for exit from shadow")
    }));
}

#[test]
fn direct_structural_member_retains_checked_owner_and_path() {
    let member_checked = crate::lower_typed_trees(lower_projection_fixture(
        r#"
                data Sample { value: f32; }
                machine member_source(sample: Sample)
                requires Float::meaning32(sample.value) == Float::meaning32(sample.value);
                {}
            "#,
    ))
    .expect("checked member source");
    let CheckedFloatProjectionSource::DirectStructuralLeaf(leaf) =
        &member_checked.facts.proof.float_meaning_projections[0].source
    else {
        panic!("direct structural member should retain checked provenance")
    };
    assert_eq!(
        member_checked.symbols.name(leaf.owner_machine),
        "member_source"
    );
    assert_eq!(leaf.field.parameter_position, 0);
    assert_eq!(
        leaf.field.path,
        [checked_trees::CheckedStructuralPredicatePathSegment::Field(
            "value".to_owned()
        )]
    );
    assert_eq!(
        leaf.fallback,
        CheckedFloatProjectionInput {
            id: CheckedFloatProjectionInputId(0),
            primitive: PrimitiveType::F32,
        }
    );
}

#[test]
fn cast_and_const_sources_remain_transitional() {
    let cast_checked = crate::lower_typed_trees(lower_projection_fixture(
        r#"
                machine cast_source(value: f32)
                requires
                    Float::meaning64(value as f64) == Float::meaning64(value as f64);
                {}
            "#,
    ))
    .expect("checked cast source");
    assert_eq!(
        cast_checked.facts.proof.float_meaning_projections[0].source,
        transitional_source(CheckedFloatProjectionInput {
            id: CheckedFloatProjectionInputId(0),
            primitive: PrimitiveType::F64,
        })
    );

    let const_checked = crate::lower_typed_trees(lower_projection_fixture(
        r#"
                machine const_source<const Value: f32>()
                requires Float::meaning32(Value) == Float::meaning32(Value);
                {}
            "#,
    ))
    .expect("checked const-parameter source");
    assert_eq!(
        const_checked.facts.proof.float_meaning_projections[0].source,
        transitional_source(CheckedFloatProjectionInput {
            id: CheckedFloatProjectionInputId(0),
            primitive: PrimitiveType::F32,
        })
    );
}

#[test]
fn nested_state_scalar_parameters_retain_direct_block_provenance() {
    let checked = crate::lower_typed_trees(lower_projection_fixture(
        r#"
                data Probe {
                }

                machine Probe::state_source(&mut self, seed: f64)
                requires
                    Float::meaning64(seed) == Float::meaning64(seed)
                {
                    transition {
                        _ -> inspect(seed)
                    }

                    state inspect(&mut self, value: f64)
                    requires
                        Float::meaning64(value) == Float::meaning64(value)
                    {}
                }
            "#,
    ))
    .expect("checked state-owned source");
    let parameter = checked
        .facts
        .proof
        .float_meaning_projections
        .iter()
        .find_map(|projection| match projection.source {
            CheckedFloatProjectionSource::DirectBlockParameter(parameter)
                if checked.symbols.name(parameter.parameter) == "value" =>
            {
                Some(parameter)
            }
            _ => None,
        })
        .expect("state scalar parameter should retain direct block provenance");
    assert_eq!(
        checked.symbols.name(parameter.owner_machine),
        "Probe::state_source"
    );
    assert_eq!(checked.symbols.name(parameter.owner_state), "inspect");
    assert_eq!(parameter.fallback.primitive, PrimitiveType::F64);
    let machine_parameter = checked
        .facts
        .proof
        .float_meaning_projections
        .iter()
        .find_map(|projection| match projection.source {
            CheckedFloatProjectionSource::DirectMachineParameter(parameter) => Some(parameter),
            _ => None,
        })
        .expect("machine parameter should retain direct provenance");
    assert_eq!(checked.symbols.name(machine_parameter.parameter), "seed");
    assert_eq!(parameter.owner_machine, machine_parameter.owner_machine);
    assert_ne!(parameter.fallback.id, machine_parameter.fallback.id);
}

#[test]
fn nested_state_contract_keeps_block_machine_and_literal_classes_disjoint() {
    let checked = crate::lower_typed_trees(lower_projection_fixture(
        r#"
                data Probe {
                }

                machine Probe::mixed(&mut self, seed: f32)
                requires
                    Float::meaning32(seed) == Float::meaning32(0.0f32)
                {
                    transition {
                        _ -> inspect(seed)
                    }

                    state inspect(&mut self, value: f32)
                    requires
                        Float::meaning32(value) == Float::meaning32(0.0f32)
                    {}
                }
            "#,
    ))
    .expect("checked mixed-ownership source");
    let proof = &checked.facts.proof;
    assert_eq!(proof.float_meaning_projections.len(), 3);
    let mut block = None;
    let mut literal = None;
    let mut machine = None;
    for projection in &proof.float_meaning_projections {
        match &projection.source {
            CheckedFloatProjectionSource::DirectBlockParameter(parameter) => {
                block = Some(*parameter)
            }
            CheckedFloatProjectionSource::ExactBinary32Literal(bits) => literal = Some(*bits),
            CheckedFloatProjectionSource::DirectMachineParameter(parameter) => {
                machine = Some(*parameter)
            }
            source => panic!("unexpected projection source: {source:?}"),
        }
    }
    let block = block.expect("state parameter should retain direct block provenance");
    let machine = machine.expect("entry parameter should retain machine provenance");
    assert_eq!(literal, Some(0.0_f32.to_bits()));
    assert_eq!(checked.symbols.name(block.parameter), "value");
    assert_eq!(checked.symbols.name(machine.parameter), "seed");
    assert_eq!(checked.symbols.name(block.owner_machine), "Probe::mixed");
    assert_eq!(block.owner_machine, machine.owner_machine);
    assert_eq!(checked.symbols.name(block.owner_state), "inspect");
    assert_ne!(
        proof.float_meaning_equalities[0].left,
        proof.float_meaning_equalities[0].right
    );
}

#[test]
fn exact_literal_bits_are_the_checked_semantic_source_identity() {
    let checked = crate::lower_typed_trees(typed_literal_projection_program()).expect("checked");
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
    let mut validation = validation::validate_specialized_program(
        &program,
        validation::OpaquePropertyValidation::Required(&[]),
    )
    .map(|validated| validated.facts)
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
    let mut validation = validation::validate_specialized_program(
        &program,
        validation::OpaquePropertyValidation::Required(&[]),
    )
    .map(|validated| validated.facts)
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
fn checked_binding_rejects_catalog_contract_tamper() {
    let program = typed_projection_program();
    let mut validation = validation::validate_specialized_program(
        &program,
        validation::OpaquePropertyValidation::Required(&[]),
    )
    .map(|validated| validated.facts)
    .expect("validate");
    validation.float_meaning_projection_invocations[0]
        .contract
        .catalog_version += 1;
    let mut proof = ProofFacts::default();
    let diagnostics = bind_float_meaning_projection_facts(
        &program,
        &mut proof,
        &validation.float_meaning_projection_invocations,
        &validation.float_meaning_equality_propositions,
    )
    .expect_err("catalog contract substitution must reject");
    assert!(
        diagnostics[0]
            .message
            .contains("contract/catalog identity drifted")
    );
    assert!(proof.float_meaning_projections.is_empty());
}

#[test]
fn checked_binding_rejects_forged_cross_format_equality_fact() {
    let mut program = typed_projection_program();
    let mut validation = validation::validate_specialized_program(
        &program,
        validation::OpaquePropertyValidation::Required(&[]),
    )
    .map(|validated| validated.facts)
    .expect("validate");
    let cross_format_right = validation.float_meaning_equality_propositions[1].right;
    let equality = &mut validation.float_meaning_equality_propositions[0];
    equality.right = cross_format_right;
    let ExpressionNode::Binary(expression) =
        program.expression_table.expression_mut(equality.expression)
    else {
        unreachable!("validated equality is binary")
    };
    expression.right = cross_format_right;
    let mut proof = ProofFacts::default();
    let diagnostics = bind_float_meaning_projection_facts(
        &program,
        &mut proof,
        &validation.float_meaning_projection_invocations,
        &validation.float_meaning_equality_propositions,
    )
    .expect_err("cross-format forged equality fact must reject");
    assert!(
        diagnostics[0]
            .message
            .contains("do not share one exact format")
    );
    assert!(proof.float_meaning_equalities.is_empty());
}

#[test]
fn source_validation_rejects_cross_format_float_meaning_equal() {
    let program = lower_projection_fixture(
        r#"
                machine prove()
                requires Float::meaning32(0.0f32) == Float::meaning64(0.0f64);
                { }
            "#,
    );
    let diagnostics = validation::validate_specialized_program(
        &program,
        validation::OpaquePropertyValidation::Required(&[]),
    )
    .map(|validated| validated.facts)
    .expect_err("FloatMeaningEqual is carrier-specific");
    assert!(
        diagnostics[0]
            .message
            .contains("same exact format and projection contract")
    );
}

#[test]
fn proof_projection_call_rejects_a_local_operator_lookalike() {
    let diagnostics = validation::validate_specialized_program(
        &local_projection_program(),
        validation::OpaquePropertyValidation::Required(&[]),
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
    let diagnostics = validation::validate_specialized_program(
        &program,
        validation::OpaquePropertyValidation::Required(&[]),
    )
    .map(|validated| validated.facts)
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
    let diagnostics = validation::validate_specialized_program(
        &program,
        validation::OpaquePropertyValidation::Required(&[]),
    )
    .map(|validated| validated.facts)
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
    let diagnostics = validation::validate_specialized_program(
        &program,
        validation::OpaquePropertyValidation::Required(&[]),
    )
    .map(|validated| validated.facts)
    .expect_err("the sealed meaning32 declaration cannot drift to binary64");
    assert!(
        diagnostics[0].message.contains("from `f32`"),
        "unexpected diagnostic: {:?}",
        diagnostics[0],
    );
}

#[test]
fn canonical_projection_declaration_rejects_public_visibility_drift() {
    let program = lower_projection_fixture_with_metadata(
        projection_source(),
        SourceOrigin::Toolchain,
        r#"
                pub operator Float::meaning32(value: f32) -> FloatMeaning;
                operator Float::meaning64(value: f64) -> FloatMeaning;
            "#,
        "float_operations.omg",
        SourceOrigin::Toolchain,
    );
    let diagnostics = validation::validate_specialized_program(
        &program,
        validation::OpaquePropertyValidation::Required(&[]),
    )
    .map(|validated| validated.facts)
    .expect_err("the sealed projection declaration is private");
    assert!(
        diagnostics[0]
            .message
            .contains("ordinary tokenless operator")
    );
}

#[test]
fn canonical_projection_declaration_rejects_contract_drift() {
    let program = lower_projection_fixture_with_metadata(
        projection_source(),
        SourceOrigin::Toolchain,
        r#"
                operator Float::meaning32(value: f32) -> FloatMeaning
                requires true == true;
                operator Float::meaning64(value: f64) -> FloatMeaning;
            "#,
        "float_operations.omg",
        SourceOrigin::Toolchain,
    );
    let diagnostics = validation::validate_specialized_program(
        &program,
        validation::OpaquePropertyValidation::Required(&[]),
    )
    .map(|validated| validated.facts)
    .expect_err("the sealed projection declaration is contract-free");
    assert!(
        diagnostics[0]
            .message
            .contains("ordinary tokenless operator")
    );
}

#[test]
fn checked_binding_rejects_validated_facts_replayed_on_a_local_lookalike() {
    let canonical = typed_projection_program();
    let validation = validation::validate_specialized_program(
        &canonical,
        validation::OpaquePropertyValidation::Required(&[]),
    )
    .map(|validated| validated.facts)
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
    let mut validation = validation::validate_specialized_program(
        &program,
        validation::OpaquePropertyValidation::Required(&[]),
    )
    .map(|validated| validated.facts)
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

#[test]
fn transported_ensures_result_instantiates_at_the_call_use_site() {
    let checked = crate::lower_typed_trees(lower_projection_fixture(
        r#"
                machine helper(value: f32) -> f32
                ensures Float::meaning32(result) == Float::meaning32(result);
                { value }

                machine caller(value: f32) -> f32
                ensures Float::meaning32(result) == Float::meaning32(result);
                { helper(value) }
            "#,
    ))
    .expect("checked");
    let proof = &checked.facts.proof;
    // The transported `ensures` instantiates once at the call: the imported
    // `result` operand names the exact produced call result, disjoint from
    // each machine's own machine-result row.
    let call_row = proof
        .float_meaning_projections
        .iter()
        .find(|projection| {
            matches!(
                projection.source,
                CheckedFloatProjectionSource::DirectCallResult(_)
            )
        })
        .expect("the imported ensures gains a call-result source");
    let CheckedFloatProjectionSource::DirectCallResult(call_result) = call_row.source else {
        unreachable!("found by source class")
    };
    assert_eq!(
        checked.symbols.name(call_result.use_site.owner_machine),
        "caller"
    );
    assert_eq!(call_result.use_site.statement_index, 0);
    assert_eq!(call_result.use_site.call_ordinal, 0);
    assert_eq!(call_result.fallback.primitive, PrimitiveType::F32);
    let machine_results = proof
        .float_meaning_projections
        .iter()
        .filter_map(|projection| match projection.source {
            CheckedFloatProjectionSource::DirectMachineResult(result) => Some(result),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(machine_results.len(), 2);
    assert!(
        machine_results
            .iter()
            .all(|result| result.fallback.id != call_result.fallback.id),
        "the call-result identity is disjoint from each machine-result row"
    );
    // The imported equality marks its use site and reflexively names the
    // call-result value; declaration rows keep `use_site: None` so the
    // (owner, expression, use site) rejoin still discharges authored exits.
    assert_eq!(proof.float_meaning_equalities.len(), 3);
    let imported = proof
        .float_meaning_equalities
        .iter()
        .filter(|equality| equality.use_site.is_some())
        .collect::<Vec<_>>();
    assert_eq!(imported.len(), 1);
    assert_eq!(imported[0].use_site, Some(call_result.use_site));
    assert_eq!(imported[0].left, imported[0].right);
    assert_eq!(imported[0].left, call_row.result.id);
    let authored = proof
        .float_meaning_equalities
        .iter()
        .filter(|equality| equality.use_site.is_none())
        .count();
    assert_eq!(authored, 2);
}

#[test]
fn transported_ensures_result_is_distinct_per_call_site() {
    let checked = crate::lower_typed_trees(lower_projection_fixture(
        r#"
                machine helper(value: f32) -> f32
                ensures Float::meaning32(result) == Float::meaning32(result);
                { value }

                machine first(value: f32) -> f32
                ensures Float::meaning32(result) == Float::meaning32(result);
                { helper(value) }

                machine second(value: f32) -> f32
                ensures Float::meaning32(result) == Float::meaning32(result);
                { helper(value) }
            "#,
    ))
    .expect("checked");
    let proof = &checked.facts.proof;
    let sites = proof
        .float_meaning_projections
        .iter()
        .filter_map(|projection| match &projection.source {
            CheckedFloatProjectionSource::DirectCallResult(result) => {
                Some((result, projection.result.id))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(sites.len(), 2);
    assert_ne!(sites[0].0.use_site, sites[1].0.use_site);
    assert_ne!(sites[0].1, sites[1].1);
    let imported = proof
        .float_meaning_equalities
        .iter()
        .filter(|equality| equality.use_site.is_some())
        .count();
    assert_eq!(imported, 2);
    let authored = proof
        .float_meaning_equalities
        .iter()
        .filter(|equality| equality.use_site.is_none())
        .count();
    assert_eq!(authored, 3);
}

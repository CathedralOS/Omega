//! Float meaning projection tests.

use super::{
    CheckedFloatProjectionInput, CheckedFloatProjectionSource, ExpressionNode,
    FloatProjectionOperation, PrimitiveType, ProofFacts, TypedTrees,
};
use crate::proof::bind_float_meaning_projection_facts;
use crate::tests::front_end::{typed_program, typed_program_from_source_map};
use checked_trees::{CheckedFloatProjectionInputId, CheckedProofValueId};
use source::{SourceMap, SourceOrigin};
use std::path::PathBuf;

const CORE_FLOAT_MEANING: &str = "pub data FloatMeaning { }";
const CORE_PROJECTIONS: &str = r#"
        operator Float::meaning32(value: f32) -> FloatMeaning;
        operator Float::meaning64(value: f64) -> FloatMeaning;
    "#;
/// The sealed toolchain `FloatFormat` record copied verbatim from
/// `source/library/core/float_format.omg` — the two permanent consts differ
/// only in their field values, which is exactly what the producer matches.
const CORE_FLOAT_FORMAT: &str = r#"
        pub data FloatSpecialValues [copy] {
            signed_zero: bool;
            subnormals: bool;
            infinity: bool;
            nan: bool;
        }
        pub data FloatFormat [copy] {
            radix: u32;
            precision: u32;
            minimum_normal_exponent: i32;
            maximum_normal_exponent: i32;
            minimum_subnormal_exponent: i32;
            specials: FloatSpecialValues;
            rounds_to_nearest_ties_to_even: bool;
        }
        pub const FloatFormat::BINARY32: FloatFormat = FloatFormat {
            radix: 2,
            precision: 24,
            minimum_normal_exponent: -126,
            maximum_normal_exponent: 127,
            minimum_subnormal_exponent: -149,
            specials: FloatSpecialValues {
                signed_zero: true,
                subnormals: true,
                infinity: true,
                nan: true,
            },
            rounds_to_nearest_ties_to_even: true,
        };
        pub const FloatFormat::BINARY64: FloatFormat = FloatFormat {
            radix: 2,
            precision: 53,
            minimum_normal_exponent: -1022,
            maximum_normal_exponent: 1023,
            minimum_subnormal_exponent: -1074,
            specials: FloatSpecialValues {
                signed_zero: true,
                subnormals: true,
                infinity: true,
                nan: true,
            },
            rounds_to_nearest_ties_to_even: true,
        };
    "#;
const CORE_SEMANTIC_PROJECTIONS: &str = r#"
        operator Float::meaning32(value: f32) -> FloatMeaning;
        operator Float::meaning64(value: f64) -> FloatMeaning;
        pub machine FloatSemantics::add(format: FloatFormat, left: FloatMeaning, right: FloatMeaning) -> FloatMeaning;
        pub machine FloatSemantics::multiply(format: FloatFormat, left: FloatMeaning, right: FloatMeaning) -> FloatMeaning;
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
    typed_program_from_source_map(
        sources,
        &[
            (meaning_source_id, CORE_FLOAT_MEANING),
            (projection_source_id, projection_declarations),
            (user_source_id, source),
        ],
    )
}

fn lower_semantic_fixture(source: &str) -> TypedTrees {
    let mut sources = SourceMap::default();
    let meaning_source_id = sources
        .add_with_metadata(
            PathBuf::from("source/library/core/float_meaning.omg"),
            CORE_FLOAT_MEANING.to_owned(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let format_source_id = sources
        .add_with_metadata(
            PathBuf::from("source/library/core/float_format.omg"),
            CORE_FLOAT_FORMAT.to_owned(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let projection_source_id = sources
        .add_with_metadata(
            PathBuf::from("source/library/core/float_operations.omg"),
            CORE_SEMANTIC_PROJECTIONS.to_owned(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let user_source_id = sources
        .add(
            PathBuf::from("tests/float_projection/main.omg"),
            source.to_owned(),
        )
        .source_id;
    typed_program_from_source_map(
        sources,
        &[
            (meaning_source_id, CORE_FLOAT_MEANING),
            (format_source_id, CORE_FLOAT_FORMAT),
            (projection_source_id, CORE_SEMANTIC_PROJECTIONS),
            (user_source_id, source),
        ],
    )
}

fn lower_local_projection_lookalike(source: &str) -> TypedTrees {
    let combined = format!("{CORE_FLOAT_MEANING}\n{CORE_PROJECTIONS}\n{source}");
    typed_program(&combined)
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
fn direct_result_reflexivity_does_not_prove_a_distinct_parameter_projection() {
    let diagnostics = crate::lower_typed_trees(
        lower_projection_fixture(
            r#"
                machine distinct(value: f32) -> f32
                ensures Float::meaning32(result) == Float::meaning32(value);
                { value }
            "#,
        ),
        &crate::CheckingRequest::settled(),
    )
    .expect_err("distinct checked projection terms require explicit evidence");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove ensures contract for exit from distinct")
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
    let diagnostics = crate::lower_typed_trees(program, &crate::CheckingRequest::settled())
        .expect_err("a real result parameter must not receive pseudo-result reflexivity");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove ensures contract for exit from shadow")
    }));
}

#[test]
fn direct_structural_member_retains_checked_owner_and_path() {
    let member_checked = crate::lower_typed_trees(
        lower_projection_fixture(
            r#"
                data Sample { value: f32; }
                machine member_source(sample: Sample)
                requires Float::meaning32(sample.value) == Float::meaning32(sample.value);
                {}
            "#,
        ),
        &crate::CheckingRequest::settled(),
    )
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
    let cast_checked = crate::lower_typed_trees(
        lower_projection_fixture(
            r#"
                machine cast_source(value: f32)
                requires
                    Float::meaning64(value as f64) == Float::meaning64(value as f64);
                {}
            "#,
        ),
        &crate::CheckingRequest::settled(),
    )
    .expect("checked cast source");
    assert_eq!(
        cast_checked.facts.proof.float_meaning_projections[0].source,
        transitional_source(CheckedFloatProjectionInput {
            id: CheckedFloatProjectionInputId(0),
            primitive: PrimitiveType::F64,
        })
    );

    let const_checked = crate::lower_typed_trees(
        lower_projection_fixture(
            r#"
                machine const_source<const Value: f32>()
                requires Float::meaning32(Value) == Float::meaning32(Value);
                {}
            "#,
        ),
        &crate::CheckingRequest::settled(),
    )
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
fn nested_state_contract_keeps_block_machine_and_literal_classes_disjoint() {
    let checked = crate::lower_typed_trees(
        lower_projection_fixture(
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
        ),
        &crate::CheckingRequest::settled(),
    )
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
    let checked = crate::lower_typed_trees(
        typed_literal_projection_program(),
        &crate::CheckingRequest::settled(),
    )
    .expect("checked");
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
    let checked = crate::lower_typed_trees(
        lower_projection_fixture(
            r#"
                machine helper(value: f32) -> f32
                ensures Float::meaning32(result) == Float::meaning32(result);
                { value }

                machine caller(value: f32) -> f32
                ensures Float::meaning32(result) == Float::meaning32(result);
                { helper(value) }
            "#,
        ),
        &crate::CheckingRequest::settled(),
    )
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
fn repeated_semantic_application_deduplicates_to_one_value() {
    let program = lower_semantic_fixture(
        r#"
            machine helper(left: f32, right: f32) -> f32
            ensures
                FloatSemantics::add(
                    FloatFormat::BINARY32,
                    Float::meaning32(left),
                    Float::meaning32(right)
                ) == FloatSemantics::add(
                    FloatFormat::BINARY32,
                    Float::meaning32(left),
                    Float::meaning32(right)
                );
            { left }
        "#,
    );
    let proof = bind_projection_facts_without_exit_proof(&program);
    let [application] = proof.float_semantic_applications.as_slice() else {
        panic!("one canonical application row per identical semantic tuple")
    };
    let reflexive = proof
        .float_meaning_equalities
        .iter()
        .find(|equality| equality.use_site.is_none())
        .expect("the application self-equality binds once");
    assert_eq!(reflexive.left, application.result);
    assert_eq!(reflexive.right, application.result);
}

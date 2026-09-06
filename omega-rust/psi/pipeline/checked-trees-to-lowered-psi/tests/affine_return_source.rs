use checked_trees::{CheckedClaimFreeAffineStructuralReturnMachinePlan, CheckedStructuralAccess};
use checked_trees_to_lowered_psi::lower_machine;
use language_semantics::{Multiplicity, SemanticDomainId};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    ClaimId, IntegerSign, IntegerType, IntegerValue, ScalarType, StructuralPlaceKind,
};
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralResult, TerminalStructuralValue,
};
use terminal_psi::{StructuralMultiplicity, Terminator};

const FREE_IDENTITY: &str = r#"
    data Value { number: u64; }
    machine forward(value: Value) -> Value { value }
"#;

const ATTACHED_IDENTITY: &str = r#"
    data Value { number: i64; }
    machine Main::forward(value: Value) -> Value { value }
"#;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let source = format!("data Main {{}} machine Main::run() {{}} {source}");
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|errors| panic!("fixture should check: {source}\n{errors:#?}"))
}

fn identity_plan_mut(
    checked: &mut checked_trees::CheckedTrees,
) -> &mut CheckedClaimFreeAffineStructuralReturnMachinePlan {
    let [plan] = checked
        .facts
        .flow
        .terminal_structural_returns
        .claim_free_affine_machines
        .as_mut_slice()
    else {
        panic!("fixture has one claim-free affine identity plan")
    };
    plan
}

fn assert_identity_execution(
    source: &str,
    name: &str,
    attached: bool,
    structural_source_position: u32,
    scalar_source_positions: &[u32],
    scalar_arguments: &[TerminalScalarValue],
) {
    let mut checked = checked(source);
    let plan = identity_plan_mut(&mut checked);
    assert_eq!(
        plan.structural_parameter.position,
        structural_source_position
    );
    assert_eq!(
        plan.scalar_parameters
            .iter()
            .map(|parameter| parameter.source_position)
            .collect::<Vec<_>>(),
        scalar_source_positions
    );
    assert_eq!(
        plan.structural_parameter.access,
        CheckedStructuralAccess::Owned
    );
    assert_eq!(plan.structural_parameter.multiplicity, Multiplicity::Affine);
    assert!(!plan.structural_parameter.is_self);
    assert!(plan.structural_parameter.qualifications.is_empty());
    assert!(plan.structural_parameter.fused_service_erasure.is_none());
    assert_eq!(
        plan.result.type_identity,
        plan.structural_parameter.type_identity
    );
    assert_eq!(plan.result.multiplicity, Multiplicity::Affine);
    assert!(plan.result.qualifications.is_empty());

    let lowered = lower_machine(&checked, name).expect("owned result producer");
    let semantic = encode_module(&lowered.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("encode proof");
    let module = decode_module(&semantic).expect("decode semantics");
    let proof_bundle = decode_proof_bundle(&proof).expect("decode proof");
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof_bundle, lowered.proof_bundle);
    terminal_verifier::verify_module(&module, &proof_bundle, &AdmissionProfile::default())
        .expect("independent verification after codec roundtrip");

    let [machine] = module.machines.as_slice() else {
        panic!("one independently produced helper")
    };
    assert_eq!(module.entry, machine.id);
    assert_eq!(machine.attachment.is_some(), attached);
    assert_eq!(machine.parameters.len(), scalar_arguments.len());
    for (parameter, argument) in machine.parameters.iter().zip(scalar_arguments) {
        let TerminalScalarValue::Integer { scalar_type, .. } = argument else {
            panic!("fixture uses fixed-width integer side arguments")
        };
        assert_eq!(parameter.scalar_type, ScalarType::Integer(*scalar_type));
    }
    let [parameter] = machine.structural_parameters.as_slice() else {
        panic!("one whole owned structural parameter")
    };
    assert_eq!(
        parameter.position, 0,
        "Terminal structural positions are dense"
    );
    assert!(!parameter.is_self);
    assert_eq!(parameter.access, terminal_psi::StructuralAccess::Owned);
    assert_eq!(parameter.multiplicity, StructuralMultiplicity::Affine);
    assert!(parameter.qualifications.is_empty());
    assert!(parameter.projected_qualifications.is_empty());
    let result = machine.result.structural().expect("structural result");
    assert_ne!(result.place, parameter.place);
    assert_eq!(result.structural_type, parameter.structural_type);
    assert_eq!(result.multiplicity, StructuralMultiplicity::Affine);
    assert!(result.qualifications.is_empty());
    assert!(result.projected_qualifications.is_empty());
    assert_eq!(machine.structural_places.len(), 2);
    assert!(machine.structural_places.iter().any(|place| {
        place.id == result.place && matches!(place.kind, StructuralPlaceKind::Result)
    }));
    assert!(machine.entry_claims.is_empty());
    assert!(machine.content_entry_claims.is_empty());
    assert!(machine.content_identity_reshuffles.is_empty());
    assert!(machine.content_partition_compositions.is_empty());
    let [block] = machine.blocks.as_slice() else {
        panic!("one whole-root return block")
    };
    assert!(block.operations.is_empty());
    let Terminator::ReturnStructural {
        source,
        returned_claims,
        trivial_affine_discards,
        ..
    } = &block.terminator
    else {
        panic!("identity returns structural custody")
    };
    assert_eq!(*source, parameter.place);
    assert!(returned_claims.is_empty());
    assert!(trivial_affine_discards.is_empty());

    let argument = TerminalStructuralValue {
        opaque_identity: 0x5eed_cafe,
        structural_type: parameter.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        scalar_arguments,
        std::slice::from_ref(&argument),
    )
    .expect("verified identity artifact starts");
    assert_eq!(execution.live_claim_frontier().count(), 0);
    let mut meter = TerminalFuelMeter::with_allowance(0);
    for _ in 0..2 {
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        assert_eq!(execution.live_claim_frontier().count(), 0);
        assert!(execution.effects().is_empty());
    }
    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
            TerminalStructuralResult {
                value: argument,
                claims: Vec::new(),
            }
        ))
    );
    assert_eq!(execution.live_claim_frontier().count(), 0);
    assert!(execution.effects().is_empty());
}

#[test]
fn an_owned_affine_identity_has_an_independent_structural_result_producer() {
    assert_identity_execution(FREE_IDENTITY, "forward", false, 0, &[], &[]);
}

#[test]
fn static_attached_affine_identity_verifies_and_resumes_without_scalar_parameters() {
    assert_identity_execution(ATTACHED_IDENTITY, "Main::forward", true, 0, &[], &[]);
}

#[test]
fn selected_affine_identity_excludes_unrelated_return_types() {
    for (source, name) in [
        (FREE_IDENTITY, "forward"),
        (ATTACHED_IDENTITY, "Main::forward"),
    ] {
        let checked = checked(&format!(
            "{source}
             data UnusedValue {{ number: u64; }}
             machine unused(value: UnusedValue) -> UnusedValue {{ value }}"
        ));
        let unused = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "unused")
            .expect("unused identity source owner");
        let plans = &checked.facts.flow.terminal_structural_returns;
        let unused_plan = plans
            .claim_free_affine_for_machine(unused.symbol)
            .expect("unused identity has a checked return plan");
        assert!(
            plans
                .structural_types
                .iter()
                .any(|declaration| declaration.identity == unused_plan.result.type_identity)
        );

        let lowered = lower_machine(&checked, name).expect("selected identity lowers");
        let module = &lowered.semantic_module;
        let [machine] = module.machines.as_slice() else {
            panic!("unused identity must not enter the selected module")
        };
        let result = machine
            .result
            .structural()
            .expect("selected structural result");
        assert!(module.structural_types.iter().any(|declaration| {
            declaration.id == result.structural_type
                && declaration.id == machine.structural_parameters[0].structural_type
        }));
        assert!(
            module.structural_types.iter().all(|declaration| {
                declaration.identity != unused_plan.result.type_identity
                    && (declaration.id == result.structural_type
                        || Some(declaration.id) == machine.attachment)
            }),
            "only the selected single-field result and optional attachment are retained"
        );
    }
}

#[test]
fn mixed_free_affine_identity_preserves_authored_parameter_positions() {
    let unsigned = TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        value: IntegerValue::Unsigned(17),
    };
    let signed = TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
        value: IntegerValue::Signed(-23),
    };
    for (parameters, structural_position, scalar_positions, arguments) in [
        ("before: u8, value: Value", 1, vec![0], vec![unsigned]),
        ("value: Value, after: i32", 0, vec![1], vec![signed]),
        (
            "before: u8, value: Value, after: i32",
            1,
            vec![0, 2],
            vec![unsigned, signed],
        ),
    ] {
        let source = format!(
            "data Value {{ number: u64; }} machine forward({parameters}) -> Value {{ value }}"
        );
        assert_identity_execution(
            &source,
            "forward",
            false,
            structural_position,
            &scalar_positions,
            &arguments,
        );
    }
}

#[test]
fn modified_checked_identity_custody_rejects_before_lowering() {
    #[derive(Clone, Copy, Debug)]
    enum Drift {
        ParameterPosition,
        BorrowedParameter,
        ResultType,
        ResultMultiplicity,
        ParameterQualification,
        ResultQualification,
        ReturnStatement,
    }
    for (source, name) in [
        (FREE_IDENTITY, "forward"),
        (ATTACHED_IDENTITY, "Main::forward"),
    ] {
        let original = checked(source);
        lower_machine(&original, name).expect("unmodified identity lowers");
        for drift in [
            Drift::ParameterPosition,
            Drift::BorrowedParameter,
            Drift::ResultType,
            Drift::ResultMultiplicity,
            Drift::ParameterQualification,
            Drift::ResultQualification,
            Drift::ReturnStatement,
        ] {
            let mut modified = original.clone();
            let plan = identity_plan_mut(&mut modified);
            match drift {
                Drift::ParameterPosition => plan.structural_parameter.position = 1,
                Drift::BorrowedParameter => {
                    plan.structural_parameter.access = CheckedStructuralAccess::SharedBorrow;
                }
                Drift::ResultType => plan.result.type_identity.push_str("::stale"),
                Drift::ResultMultiplicity => plan.result.multiplicity = Multiplicity::Linear,
                Drift::ParameterQualification => {
                    plan.structural_parameter
                        .qualifications
                        .push(SemanticDomainId(99));
                }
                Drift::ResultQualification => plan.result.qualifications.push(SemanticDomainId(99)),
                Drift::ReturnStatement => plan.return_statement_ordinal = 1,
            }
            assert!(lower_machine(&modified, name).is_err(), "{name}: {drift:?}");
        }
    }
}

#[test]
fn modified_mixed_identity_parameter_partition_rejects() {
    let original = checked(
        "data Value { number: u64; } machine forward(before: u8, value: Value, after: i32) -> Value { value }",
    );
    lower_machine(&original, "forward").expect("unmodified mixed identity lowers");
    for coordinated in [false, true] {
        let mut modified = original.clone();
        let plan = identity_plan_mut(&mut modified);
        plan.scalar_parameters[0].source_position = 1;
        if coordinated {
            plan.structural_parameter.position = 0;
        }
        assert!(
            lower_machine(&modified, "forward").is_err(),
            "authored partition must rejoin even when the positions remain unique: {coordinated}"
        );
    }
}

#[test]
fn altered_terminal_identity_returns_fail_independent_verification() {
    #[derive(Clone, Copy, Debug)]
    enum Drift {
        UninitializedResultSource,
        InventedClaim,
        DiscardedReturnedParameter,
    }
    let lowered = lower_machine(&checked(FREE_IDENTITY), "forward").expect("identity lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("unmodified identity verifies");
    for drift in [
        Drift::UninitializedResultSource,
        Drift::InventedClaim,
        Drift::DiscardedReturnedParameter,
    ] {
        let mut module = lowered.semantic_module.clone();
        let machine = &mut module.machines[0];
        let result_place = machine.result.structural().unwrap().place;
        let parameter_place = machine.structural_parameters[0].place;
        let Terminator::ReturnStructural {
            source,
            returned_claims,
            trivial_affine_discards,
            ..
        } = &mut machine.blocks[0].terminator
        else {
            panic!("identity return")
        };
        match drift {
            Drift::UninitializedResultSource => *source = result_place,
            Drift::InventedClaim => returned_claims.push(ClaimId::new(1).unwrap()),
            Drift::DiscardedReturnedParameter => trivial_affine_discards.push(parameter_place),
        }
        assert!(
            terminal_verifier::verify_module(
                &module,
                &lowered.proof_bundle,
                &AdmissionProfile::default(),
            )
            .is_err(),
            "{drift:?} must fail independent return-custody verification"
        );
    }
}

#[test]
fn unsupported_reference_cleanup_and_linear_identities_have_no_affine_plan() {
    for source in [
        "data Value { number: u64; } machine forward(value: &Value) -> &Value { value }",
        "data Value { number: u64; } machine forward(value: &mut Value) -> &mut Value { value }",
        "data Value { number: u64; } machine Value::drop(&mut self) {} machine forward(value: Value) -> Value { value }",
        "data Value [linear] { number: u64; } machine forward(value: Value) -> Value { value }",
        "data Resource [linear] { number: u64; } data Value { resource: Resource; } machine forward(value: Value) -> Value { value }",
    ] {
        let checked = checked(source);
        assert!(
            checked
                .facts
                .flow
                .terminal_structural_returns
                .claim_free_affine_machines
                .is_empty(),
            "unsupported ownership must not enter the claim-free affine producer: {source}"
        );
        assert!(lower_machine(&checked, "forward").is_err(), "{source}");
    }
}

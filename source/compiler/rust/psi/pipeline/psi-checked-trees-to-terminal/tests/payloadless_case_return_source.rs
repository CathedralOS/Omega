use psi_proof_admission::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{OperationKind, OperationResult, StructuralTypeShape, Terminator};
use psi_terminal_codec::{decode_module, encode_module, encode_proof_bundle};
use psi_terminal_fixed_fuel::derive_fixed_entry_fuel;
use psi_terminal_fuel::TerminalFuelMeter;
use psi_terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalPayloadlessCaseResult, TerminalPayloadlessCaseValue,
};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    data Outcome [copy] {
        case Success;
        case Failure;
    }
    data Root {}

    machine Root::choose() -> Outcome {
        Outcome::Success
    }
"#;

fn checked_source() -> psi_checked_trees::CheckedTrees {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

#[test]
fn exact_payloadless_case_return_is_canonical_verified_and_executable() {
    let checked = checked_source();
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::choose")
        .expect("the exact payloadless case producer lowers");
    let module = &lowered.semantic_module;
    let [machine] = module.machines.as_slice() else {
        panic!("the source producer lowers to one terminal machine")
    };
    let [block] = machine.blocks.as_slice() else {
        panic!("the source producer lowers to one terminal block")
    };
    let [operation] = block.operations.as_slice() else {
        panic!("payloadless construction is one exact structural operation")
    };
    let OperationResult::Structural(operation_result) = &operation.result else {
        panic!("the payloadless case operation must establish a structural place")
    };
    assert!(operation_result.qualifications.is_empty());
    assert!(operation_result.claims.is_empty());
    let OperationKind::EstablishPayloadlessCase { result_case } = &operation.kind else {
        panic!("the source case constructor must remain exact")
    };
    let result_type = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == operation_result.structural_type)
        .expect("the operation result type is declared");
    let StructuralTypeShape::Sum { cases } = &result_type.shape else {
        panic!("the operation result remains a sum")
    };
    assert_eq!(
        cases
            .iter()
            .find(|case| case.id == *result_case)
            .map(|case| case.identity.as_str()),
        Some("Success")
    );
    let Terminator::ReturnStructural {
        source,
        returned_claims,
        trivial_affine_discards,
        ..
    } = &block.terminator
    else {
        panic!("the constructed place returns through structural custody")
    };
    assert_eq!(*source, operation_result.place);
    assert!(returned_claims.is_empty());
    assert!(trivial_affine_discards.is_empty());

    let bytes = encode_module(module).expect("payloadless case semantics encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let verified = psi_terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("payloadless case construction verifies independently");
    let fixed = derive_fixed_entry_fuel(&verified, module.entry)
        .expect("the exact source producer has fixed fuel");
    assert_eq!(fixed.ceiling_units(), 2);

    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof bundle encodes");
    let mut execution =
        TerminalExecution::start_artifact(&bytes, &proof, &AdmissionProfile::default(), &[])
            .expect("the payloadless case artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution
            .resume(&mut meter)
            .expect("operation exhaustion is resumable"),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).expect("fund the case constructor");
    assert!(matches!(
        execution
            .resume(&mut meter)
            .expect("return exhaustion is resumable"),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).expect("fund the return edge");
    assert_eq!(
        execution
            .resume(&mut meter)
            .expect("the case return completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::PayloadlessCase(
            TerminalPayloadlessCaseResult {
                value: TerminalPayloadlessCaseValue {
                    structural_type: operation_result.structural_type,
                    result_case: *result_case,
                },
            }
        ))
    );
}

//! Empty scalar contracts must agree with authored clauses and parameter ranges.

use checked_trees::{CheckedTrees, ClosedScalarValueContractPlan};
use checked_trees_to_lowered_psi::LoweringError;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

fn publish(checked: &CheckedTrees) -> (CanonicalTerminalArtifact, terminal_psi::TerminalModule) {
    let artifact = terminal_production::TerminalProductionRequest::new(
        checked,
        TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("unmodified source must publish before custody mutations")
    .into_artifact();
    let module =
        terminal_codec::decode_module(artifact.semantic_bytes()).expect("reload semantics");
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).expect("reload proof");
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("independently verify the reloaded artifact");
    (artifact, module)
}

fn execute_identity(artifact: &CanonicalTerminalArtifact) {
    let input = TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(7),
    };
    assert_eq!(
        interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &AdmissionProfile::default(),
            &[input],
        )
        .expect("execute source identity from the artifact"),
        TerminalExecutionResult::Scalar(input),
    );
}

fn reject_erased_contract(original: &CheckedTrees, owner: &str, expected_message: &str) {
    let mut changed = original.clone();
    let symbol = changed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == owner)
        .expect("authored contract owner")
        .symbol;
    let contract = changed
        .facts
        .contract_plans
        .machines
        .iter_mut()
        .find(|contract| contract.machine == symbol)
        .expect("retained scalar contract");
    assert!(
        !contract.closed_scalar_values.requires().is_empty()
            || !contract.closed_scalar_values.ensures().is_empty(),
        "mutation must delete actual checked contract rows"
    );
    contract.closed_scalar_values = ClosedScalarValueContractPlan::default();
    assert_eq!(
        changed.typed, original.typed,
        "authored custody is unchanged"
    );
    match checked_trees_to_lowered_psi::lower_machine(
        &changed,
        TerminalMachineSelection::Name("enter"),
    ) {
        Err(LoweringError::Unsupported(message)) => assert_eq!(message, expected_message),
        Err(error) => panic!("unexpected rejection for {owner}: {error:#?}"),
        Ok(_) => panic!("accepted erased scalar contract for {owner}"),
    }
    assert!(
        terminal_production::TerminalProductionRequest::new(
            &changed,
            TerminalMachineSelection::Name("enter")
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default()
        ))
        .is_err(),
        "erased scalar contract must not publish"
    );
}

#[test]
fn empty_checked_contract_cannot_erase_authored_normal_clauses() {
    // Erased requires rows are caught by the parameter-range rejoin's authored
    // count before contract selection; an ensures-only erasure still reaches
    // the empty-contract source check.
    for (clauses, requires, ensures, erased_message) in [
        (
            "requires input <= 11u64",
            1,
            0,
            "scalar contract lost authored requirements",
        ),
        (
            "ensures result == input",
            0,
            1,
            "empty scalar contract would erase an authored normal clause",
        ),
        (
            "requires input <= 11u64\nensures result == input",
            1,
            1,
            "scalar contract lost authored requirements",
        ),
    ] {
        let source = format!("machine enter(input: u64) -> u64\n{clauses}\n{{ input }}");
        let original = crate::front_end::checked_program(&source);
        let (artifact, module) = publish(&original);
        assert_eq!(module.machines.len(), 1);
        assert_eq!(module.machines[0].contract.requires.len(), requires);
        assert_eq!(module.machines[0].contract.ensures.len(), ensures);
        execute_identity(&artifact);
        reject_erased_contract(&original, "enter", erased_message);
    }
}

#[test]
fn empty_checked_contract_cannot_erase_implicit_parameter_ranges() {
    for parameter_type in ["u64 [0..=11]", "u64 [0..12]"] {
        let source = format!("machine enter(input: {parameter_type}) -> u64 {{ input }}");
        let original = crate::front_end::checked_program(&source);
        let source_machine = &original.machines()[0];
        assert!(original.machine_contracts(source_machine).is_empty());
        let (artifact, module) = publish(&original);
        assert_eq!(module.machines.len(), 1);
        let contract = &module.machines[0].contract;
        assert_eq!(
            contract.requires.len(),
            1,
            "range is a real entry requirement"
        );
        assert!(!matches!(
            contract.requires[0],
            semantic_vocabulary::Proposition::Truth
        ));
        assert!(contract.ensures.is_empty());
        execute_identity(&artifact);
        // An erased range tail is caught by the rejoin's bound-pair walk
        // before contract selection can inspect the empty plan.
        reject_erased_contract(
            &original,
            "enter",
            "scalar entry range lost its two ordered bounds",
        );
    }
}

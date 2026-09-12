use super::*;
use compiler::CheckedCompileRequest;

#[test]
fn checked_source_booleans_survive_frontend_drop() {
    let checked = compile_to_checked(CheckedCompileRequest::new(&source_canary(), None))
        .expect("terminal-Psi Boolean source canary should compile");
    let constant = lower_machine(&checked, "terminal_boolean_constant")
        .expect("Boolean constant source should lower");
    let parameter = lower_machine(&checked, "terminal_ninth_boolean")
        .expect("Boolean parameter source should lower");
    let chain = lower_machine(&checked, "terminal_boolean_chain")
        .expect("Boolean state chain should lower");
    drop(checked);

    let constant_verified = verify_module(
        &constant.semantic_module,
        &constant.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source Boolean constant should verify");
    let constant_fuel = derive_fixed_entry_fuel(&constant_verified, constant.semantic_module.entry)
        .expect("Boolean constant should have fixed fuel");
    assert_eq!(constant_fuel.ceiling_units(), 2);
    let constant_result = interpret_verified_artifact(&constant_verified, &[])
        .expect("source Boolean constant should execute");
    assert_eq!(
        constant_result.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
    );
    assert_eq!(constant_result.usage().total_units(), 2);

    let parameter_verified = verify_module(
        &parameter.semantic_module,
        &parameter.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source Boolean parameter should verify");
    let parameter_fuel =
        derive_fixed_entry_fuel(&parameter_verified, parameter.semantic_module.entry)
            .expect("Boolean parameter should have fixed fuel");
    assert_eq!(parameter_fuel.ceiling_units(), 1);
    let arguments = [false, false, false, false, false, false, false, false, true]
        .into_iter()
        .map(TerminalScalarValue::Boolean)
        .collect::<Vec<_>>();
    let parameter_result = interpret_verified_artifact(&parameter_verified, &arguments)
        .expect("source Boolean parameter should execute");
    assert_eq!(
        parameter_result.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
    );
    assert_eq!(parameter_result.usage().total_units(), 1);

    let chain_verified = verify_module(
        &chain.semantic_module,
        &chain.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source Boolean state chain should verify");
    let chain_fuel = derive_fixed_entry_fuel(&chain_verified, chain.semantic_module.entry)
        .expect("Boolean state chain should have fixed fuel");
    assert_eq!(chain_fuel.ceiling_units(), 3);
    let chain_result = interpret_verified_artifact(&chain_verified, &arguments)
        .expect("source Boolean state chain should execute");
    assert_eq!(
        chain_result.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
    );
    assert_eq!(chain_result.usage().total_units(), 3);
}

#[cfg(unix)]
#[test]
fn source_boolean_jump_bindings_reach_stack_parameter_target_control() {
    let checked = compile_to_checked(CheckedCompileRequest::new(&source_canary(), None))
        .expect("terminal-Psi Boolean state-chain canary should compile");
    let lowered = lower_machine(&checked, "terminal_boolean_chain")
        .expect("Boolean state chain should lower");
    drop(checked);

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("Boolean state chain should verify");
    let abstract_operations = lower_verified_artifact(&verified)
        .expect("Boolean jump bindings should lower without frontend state");
    let _target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("Boolean jump bindings should select for the host");
}

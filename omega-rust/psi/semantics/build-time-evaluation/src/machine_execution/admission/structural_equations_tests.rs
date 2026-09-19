use super::BuildTimeAdmissionPlan;
use crate::machine_execution::build_machines::{
    BuildMachineExecutionMode, BuildMachineInvocation, PreparedBuildMachine,
    PreparedBuildMachineProgram, evaluate_build_machine_measured,
};
use source_files_to_tokens::Lexer;

#[test]
fn provisional_wrapper_cannot_execute_a_specialized_pending_equation() {
    let source = "machine extent<Array, Element, const Count: u64>() -> u64
        where Array == [Element; Count] { Count }
        machine wrapper() -> u64 { extent<[u8; 7], u8, 8>() }
        const ANSWER: u64 = 5;
        machine independent() -> u64 { ANSWER }";
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::pre_resolution::resolve_numeric_probe(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let prepared = PreparedBuildMachineProgram::prepare(&typed).unwrap();
    assert!(
        prepared
            .typed()
            .machines()
            .iter()
            .any(|machine| machine.type_parameters.is_empty()
                && machine.structural_type_equations_pending),
        "specialization preserves its unresolved equation"
    );
    let wrapper = prepared
        .typed()
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "wrapper")
        .unwrap();
    let admission = BuildTimeAdmissionPlan::infer(prepared.typed(), None);
    let rejection = admission
        .require_discharged_structural_equations(prepared.typed(), wrapper.symbol)
        .unwrap_err();
    assert!(
        rejection.contains("undischarged structural type equation"),
        "{rejection}"
    );
    let rejection = evaluate_build_machine_measured(
        &prepared,
        BuildMachineInvocation {
            machine: PreparedBuildMachine::Name("wrapper"),
            arguments: Vec::new(),
            mode: BuildMachineExecutionMode::Pure,
            sponsor: None,
        },
    )
    .unwrap_err();
    assert!(
        rejection
            .to_string()
            .contains("undischarged structural type equation"),
        "{rejection}"
    );
    assert!(
        evaluate_build_machine_measured(
            &prepared,
            BuildMachineInvocation {
                machine: PreparedBuildMachine::Name("independent"),
                arguments: Vec::new(),
                mode: BuildMachineExecutionMode::Pure,
                sponsor: None,
            }
        )
        .is_ok(),
        "an unrelated pending template does not reject a closed invocation"
    );
}

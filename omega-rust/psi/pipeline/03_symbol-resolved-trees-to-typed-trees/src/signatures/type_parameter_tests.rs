use symbol_resolved_trees as resolved;
const INTERLEAVED_FAMILIES: &str = r#"
    machine apply<T, machine First, const Count: u64, machine Second>(value: u64) -> u64
    where machine First<const Number: u64>(value: u64) -> u64;
    where machine Second<const Number: u64>(value: u64) -> u64;
    { value }
"#;

#[test]
fn nested_callable_telescope_still_rejects_an_unresolved_sibling_contract() {
    let mut resolved = crate::front_end::resolved_program(INTERLEAVED_FAMILIES);
    let parameters = resolved
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "apply")
        .unwrap()
        .type_parameters;
    resolved
        .tables
        .declarations
        .data_type_parameters
        .span_mut_or_empty(parameters)[3]
        .kind = resolved::data::TypeParameterKind::Machine {
        contract: resolved::data::MachineParameterContract::AuthoredNominal {
            requirement: Vec::new(),
        },
    };
    let diagnostics = crate::lower_symbol_resolved_trees(&resolved)
        .expect_err("unresolved contract is not an empty telescope");
    assert!(
        format!("{diagnostics:?}").contains("unresolved nominal machine-parameter requirement")
    );
}

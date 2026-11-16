//! The ordinary source producer must retain a whole bodyless header, including
//! the domain requirements generated for qualified formals.

use language_core::sealed_intrinsics::{CompilerPrimitive, RunCustodyOperation, SealedIntrinsic};
use typed_trees::domain::ProofFact;
use typed_trees::expression::ExpressionNode;

#[test]
fn real_run_headers_retain_every_qualified_formal_in_the_bodyless_contract() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../../source/library/core");
    let mut paths = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "omg"))
        .collect::<Vec<_>>();
    paths.sort_unstable();
    let units = paths
        .iter()
        .map(|path| {
            (
                path.file_name().unwrap().to_str().unwrap().to_owned(),
                std::fs::read_to_string(path).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let units = units
        .iter()
        .map(|(path, text)| (0, path.as_str(), text.as_str()))
        .collect::<Vec<_>>();
    let program = super::qualified_operator_home_tests::packages(&units, &[], &[])
        .expect("the complete actual bundled core frontier resolves and types");
    let mut checked = Vec::new();
    for machine in program.machines() {
        let Some(SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::RunCustody(operation))) =
            program
                .symbols
                .sealed_declaration(machine.symbol, machine.name.as_str())
        else {
            continue;
        };
        assert!(
            matches!(program.compiler_primitive_schema(machine.symbol).unwrap_or_else(|reason| panic!("{}: {reason:?}", machine.name)), Some((CompilerPrimitive::RunCustody(actual), _)) if actual == operation)
        );
        let (_, signature) = program
            .bodyless_declaration(machine.symbol)
            .unwrap_or_else(|reason| panic!("{}: {reason}", machine.name))
            .expect("the exact compiler declaration has a complete bodyless header");
        let parameters = program.state_parameters.span(signature.parameters).unwrap();
        let mut qualified = Vec::new();
        for contract in program
            .signature_contracts
            .span(signature.contracts)
            .unwrap()
        {
            for fact in program.proof_facts.span(contract.facts).unwrap() {
                let ProofFact::Membership(membership) = fact else {
                    continue;
                };
                let ExpressionNode::Name(subject) =
                    program.expression_table.expression(membership.value)
                else {
                    panic!("the original formal subject must survive");
                };
                assert!(
                    parameters
                        .iter()
                        .any(|parameter| parameter.symbol == subject.symbol)
                );
                assert!(
                    program
                        .domain_definition(membership.domain_symbol)
                        .is_some()
                );
                qualified.push(subject.symbol);
            }
        }
        let expected = match operation {
            RunCustodyOperation::ReleaseEmpty | RunCustodyOperation::Split => 1,
            RunCustodyOperation::Merge => 2,
        };
        assert_eq!(
            qualified.len(),
            expected,
            "{} lost a qualified input",
            machine.name
        );
        assert_eq!(qualified[0], parameters[0].symbol);
        if expected == 2 {
            assert_eq!(qualified[1], parameters[1].symbol);
        }
        checked.push(operation);
    }
    checked.sort_unstable_by_key(|operation| operation.tag());
    let mut expected = [
        RunCustodyOperation::ReleaseEmpty,
        RunCustodyOperation::Split,
        RunCustodyOperation::Merge,
    ];
    expected.sort_unstable_by_key(|operation| operation.tag());
    assert_eq!(checked, expected);
}

#[test]
fn a_bodyful_machine_keeps_its_qualified_formal_requirement_with_its_state() {
    let program = super::qualified_operator_home_tests::checked(&[(
        "body.omg",
        "domain u32::Tagged; machine value(input: u32 in Tagged) -> u32 in Tagged { input }",
    )])
    .expect("an ordinary qualified machine types");
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    assert!(machine.body_is_present);
    assert!(
        program
            .machine_states(machine)
            .iter()
            .flat_map(|state| program.signature_contracts.span_or_empty(state.contracts))
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
            .any(|fact| matches!(fact, ProofFact::Membership(_)))
    );
    assert!(
        !program
            .signature_contracts
            .span_or_empty(machine.contracts)
            .iter()
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
            .any(|fact| matches!(fact, ProofFact::Membership(_)))
    );
}

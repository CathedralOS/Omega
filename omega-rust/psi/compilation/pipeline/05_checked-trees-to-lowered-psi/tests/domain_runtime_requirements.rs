//! Runtime domain predicates retain their authored membership and subject.
use checked_trees::{CheckedScalarExpression, CheckedTrees};
use checked_trees_to_lowered_psi::{TerminalMachineSelection, lower_machine};
use typed_trees::domain::ProofFact;

const SOURCE: &str = r#"
domain i32::Window(value) requires value >= 2, value <= 4;
domain i32::SameWindow(value) requires value >= 2, value <= 4;
data Scalar {}
data Sink {}
machine Sink::finish(value: i32) {}
machine Scalar::measure(value: i32, other: i32) -> i32
requires value in i32::Window, other == other
{
    Sink::finish(other);
    value
}
data Main {}
machine Main::main(&mut self) {
    let result: i32 = Scalar::measure(3, 7);
    Sink::finish(result);
}
"#;

fn target(checked: &CheckedTrees) -> symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Scalar::measure")
        .expect("scalar machine")
        .symbol
}

fn verify(checked: &CheckedTrees) {
    let lowered = lower_machine(checked, TerminalMachineSelection::Name("Main::main"))
        .expect("ordinary attached scalar membership lowers");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let evidence =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
    let module = terminal_codec::decode_module(&semantic).unwrap();
    let proof = terminal_codec::decode_proof_bundle(&evidence).unwrap();
    assert_eq!(module, lowered.semantic_module);
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("reloaded membership contract verifies independently");
}

#[test]
fn domain_runtime_requirements_preserve_conjunctive_and_qualified_entry_contracts() {
    for source in [
        SOURCE.to_owned(),
        SOURCE.replace("value: i32, other", "value: i32 in Window, other"),
    ] {
        let checked = crate::front_end::checked_program(&source);
        let contract = checked
            .facts
            .contract_plans
            .for_machine(target(&checked))
            .unwrap();
        assert!(!contract.crash.runtime_membership_subjects().is_empty());
        assert!(
            contract
                .crash
                .structural_runtime_requirements()
                .unwrap()
                .len()
                >= 3
        );
        verify(&checked);
    }
}

#[test]
fn domain_runtime_requirements_reject_changed_subject_identity_and_predicate_roster() {
    let original = crate::front_end::checked_program(SOURCE);
    verify(&original);
    let machine = target(&original);
    let sibling = original
        .typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "i32::SameWindow")
        .unwrap()
        .clone();
    for mutation in 0..8 {
        let mut changed = original.clone();
        let contract = changed
            .facts
            .contract_plans
            .machines
            .iter_mut()
            .find(|contract| contract.machine == machine)
            .unwrap();
        let mut subjects = contract.crash.runtime_membership_subjects().to_vec();
        let mut requirements = contract
            .crash
            .structural_runtime_requirements()
            .unwrap()
            .to_vec();
        assert_eq!(subjects.len(), 1);
        assert_eq!(requirements.len(), 3);
        match mutation {
            0 => {
                subjects[0].subject = CheckedScalarExpression::Parameter {
                    position: 1,
                    primitive_type: checked_trees::types::PrimitiveType::I32,
                }
            }
            1 => subjects[0].fact_ordinal += 1,
            2 => {
                subjects[0].membership.domain_symbol = sibling.symbol;
                subjects[0].membership.semantic_domain = sibling.semantic_id;
            }
            3 => subjects.clear(),
            4 => subjects.push(subjects[0].clone()),
            5 => {
                requirements.remove(0);
            }
            6 => requirements.swap(0, 1),
            7 => requirements[0] = checked_trees::CheckedBooleanExpression::Constant(true),
            _ => unreachable!(),
        }
        contract.crash = contract
            .crash
            .clone()
            .with_runtime_membership_subjects(subjects)
            .with_structural_runtime_requirements(Some(requirements));
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("Main::main")).is_err(),
            "membership mutation {mutation}"
        );
    }
}

#[test]
fn domain_runtime_requirements_reject_equal_predicate_source_domain_substitution() {
    let mut changed = crate::front_end::checked_program(SOURCE);
    verify(&changed);
    let original = changed
        .facts
        .contract_plans
        .for_machine(target(&changed))
        .unwrap()
        .crash
        .runtime_membership_subjects()[0]
        .membership;
    let sibling = changed
        .typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "i32::SameWindow")
        .unwrap()
        .clone();
    let handles = changed
        .typed
        .proof_facts
        .iter()
        .filter_map(|(handle, fact)| {
            matches!(fact, ProofFact::Membership(membership) if *membership == original)
                .then_some(handle)
        })
        .collect::<Vec<_>>();
    assert_eq!(handles.len(), 1);
    let ProofFact::Membership(membership) = changed.typed.proof_facts.get_mut(handles[0]) else {
        unreachable!()
    };
    membership.domain_symbol = sibling.symbol;
    membership.semantic_domain = sibling.semantic_id;
    assert!(lower_machine(&changed, TerminalMachineSelection::Name("Main::main")).is_err());
}

#[test]
fn domain_runtime_requirements_cannot_replace_route_authority_by_predicates() {
    let mut changed = crate::front_end::checked_program(SOURCE);
    verify(&changed);
    let machine = target(&changed);
    let domain = changed
        .typed
        .domain_definitions
        .iter()
        .find_map(|(handle, domain)| (domain.name.as_str() == "i32::Window").then_some(handle))
        .unwrap();
    // A retained predicate capsule must not turn a now-routed declaration into
    // ordinary Boolean evidence, even when its predicate text is unchanged.
    changed
        .typed
        .domain_definitions
        .get_mut(domain)
        .establishment_routes
        .push(language_semantics::DomainEstablishmentRoute::ExactMachine { machine });
    assert!(lower_machine(&changed, TerminalMachineSelection::Name("Main::main")).is_err());
}

#[test]
fn domain_runtime_requirements_unit_signatures_rejoin_membership_subjects() {
    let source = SOURCE
        .replace(
            "Scalar::measure(value: i32, other: i32) -> i32",
            "Scalar::measure(value: i32, other: i32)",
        )
        .replace(
            "    Sink::finish(other);\n    value",
            "    Sink::finish(other);",
        )
        .replace(
            "    let result: i32 = Scalar::measure(3, 7);\n    Sink::finish(result);",
            "    Scalar::measure(3, 7);",
        );
    let original = crate::front_end::checked_program(&source);
    verify(&original);
    let machine = target(&original);
    let sibling = original
        .typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "i32::SameWindow")
        .unwrap()
        .clone();
    for mutation in 0..3 {
        let mut changed = original.clone();
        let contract = changed
            .facts
            .contract_plans
            .machines
            .iter_mut()
            .find(|contract| contract.machine == machine)
            .unwrap();
        let mut subjects = contract.crash.runtime_membership_subjects().to_vec();
        assert_eq!(subjects.len(), 1);
        match mutation {
            0 => {
                subjects[0].subject = CheckedScalarExpression::Parameter {
                    position: 1,
                    primitive_type: checked_trees::types::PrimitiveType::I32,
                }
            }
            1 => {
                subjects[0].membership.domain_symbol = sibling.symbol;
                subjects[0].membership.semantic_domain = sibling.semantic_id;
            }
            2 => subjects.clear(),
            _ => unreachable!(),
        }
        contract.crash = contract
            .crash
            .clone()
            .with_runtime_membership_subjects(subjects);
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("Main::main")).is_err(),
            "Unit membership mutation {mutation}"
        );
    }
}

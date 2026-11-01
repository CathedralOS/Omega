use super::collect;
use crate::{
    CheckFacts, CheckedAcceptedTemplateClassification, CheckedAcceptedTemplateIdentities,
    CheckedAcceptedTemplateIdentity, CheckedTrees, MachineContractCommitment, MachineContractPlan,
    PreparedProviderInventory,
};
use language_semantics::trust_declarations::TrustDeclarationCardinality;
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;
use typed_trees::typed_trees::{MachineSpecialization, MachineTemplateCommitment};

fn machine(index: u32, name: &str) -> Machine {
    Machine {
        symbol: SymbolHandle::from_arena_index(index),
        name: name.into(),
        supply_mode: language_semantics::MachineSupplyMode::AdmissionClaim,
        ..Default::default()
    }
}

#[test]
fn complete_census_retains_exact_evidence_and_excludes_fresh_specialized_instances() {
    let mut typed = TypedTrees::default();
    typed.push_machine(machine(1, "Template"));
    typed.push_machine(machine(2, "Instance"));
    typed.push_machine(Machine {
        supply_mode: language_semantics::MachineSupplyMode::CheckedBody,
        ..machine(3, "Ordinary")
    });
    typed.push_machine(machine(4, "ReusedTemplate"));
    for (template, instance) in [(1, 2), (4, 4)] {
        typed.machine_specializations.push(MachineSpecialization {
            template: SymbolHandle::from_arena_index(template),
            instance: SymbolHandle::from_arena_index(instance),
            accepted_template_commitment: Some("retained source commitment".to_owned()),
            ..Default::default()
        });
    }
    let mut checked = CheckedTrees::with_roots(typed, CheckFacts::default());
    checked.facts.accepted_template_identities =
        CheckedAcceptedTemplateIdentities::with_rows(vec![
            CheckedAcceptedTemplateClassification {
                machine: SymbolHandle::from_arena_index(1),
                identity: Some(CheckedAcceptedTemplateIdentity {
                    report_fingerprint: 71,
                    commitment: MachineTemplateCommitment::from_digest([11; 32]),
                }),
            },
            CheckedAcceptedTemplateClassification {
                machine: SymbolHandle::from_arena_index(4),
                identity: None,
            },
        ]);
    checked
        .facts
        .contract_plans
        .machines
        .push(MachineContractPlan {
            machine: SymbolHandle::from_arena_index(1),
            closed_scalar_values: Default::default(),
            crash: Default::default(),
            report_fingerprint: 37,
            commitment: MachineContractCommitment::from_digest([7; 32]),
        });
    let declarations = collect(&checked);
    assert_eq!(declarations.machines.len(), 2);
    assert_eq!(
        declarations.machines[0].machine,
        SymbolHandle::from_arena_index(1)
    );
    let template = declarations.machines[0].template.unwrap().unwrap();
    assert_eq!(template.report_fingerprint, 71);
    assert_eq!(template.commitment, [11; 32]);
    let contract = declarations.machines[0].contract.unwrap();
    assert_eq!(contract.report_fingerprint, 37);
    assert_eq!(contract.commitment, [7; 32]);
    assert_eq!(
        declarations.machines[1].machine,
        SymbolHandle::from_arena_index(4)
    );
    assert_eq!(declarations.machines[1].template, Ok(None));
    assert_eq!(
        declarations.machines[1].contract,
        Err(TrustDeclarationCardinality::Missing)
    );
}

#[test]
fn duplicate_classifications_and_contracts_are_deferred_without_selecting_the_first() {
    let mut typed = TypedTrees::default();
    typed.push_machine(machine(1, "Claim"));
    typed.push_machine(machine(2, "Unclassified"));
    let mut checked = CheckedTrees::with_roots(typed, CheckFacts::default());
    let classification = CheckedAcceptedTemplateClassification {
        machine: SymbolHandle::from_arena_index(1),
        identity: None,
    };
    checked.facts.accepted_template_identities =
        CheckedAcceptedTemplateIdentities::with_rows(vec![classification.clone(), classification]);
    let contract = MachineContractPlan {
        machine: SymbolHandle::from_arena_index(1),
        closed_scalar_values: Default::default(),
        crash: Default::default(),
        report_fingerprint: 37,
        commitment: MachineContractCommitment::from_digest([7; 32]),
    };
    checked
        .facts
        .contract_plans
        .machines
        .extend([contract.clone(), contract]);
    let declarations = collect(&checked);
    assert_eq!(
        declarations.machines[0].template,
        Err(TrustDeclarationCardinality::Duplicate)
    );
    assert_eq!(
        declarations.machines[0].contract,
        Err(TrustDeclarationCardinality::Duplicate)
    );
    assert_eq!(
        declarations.machines[1].template,
        Err(TrustDeclarationCardinality::Missing)
    );
    assert_eq!(
        declarations.machines[1].contract,
        Err(TrustDeclarationCardinality::Missing)
    );
}

#[test]
fn an_inventory_publishes_one_trust_roster_for_its_exact_immutable_owner() {
    let checked = Arc::new(CheckedTrees::default());
    let inventory = PreparedProviderInventory::new(Arc::clone(&checked));
    assert!(std::ptr::eq(
        inventory.trust_declarations(),
        inventory.trust_declarations()
    ));
    assert!(Arc::ptr_eq(inventory.program(), &checked));
}

use super::*;
use crate::machine_inputs::PreparedMachineInputs;

#[test]
fn only_declared_specializations_lose_authored_branch_diagnostics() {
    let guarded_machine = || {
        machine(
            vec![u32_type()],
            vec![
                guarded(
                    compare(Strict::Less, slot(0, u32_type()), unsigned(5)),
                    unsigned(1),
                ),
                returns(unsigned(0)),
            ],
        )
    };
    let mut elaboration = lower(vec![
        guarded_machine(),
        guarded_machine(),
        guarded_machine(),
    ])
    .unwrap();
    assert!(!elaboration.sources.get(id(2)).unwrap().branches.is_empty());
    // This test varies source classification metadata only. Acceptance of the
    // generated declaration and its proofs remains the kernel path's job.
    let mut identities = elaboration.module.identities.clone().into_builder();
    let selected = identities.intern("generated-source-classification");
    let nominal = terminal_psi::NominalDeclarationIdentity::new(selected);
    elaboration.module.identities = identities.build();
    elaboration.module.machines[1].contract.declaration =
        Some(terminal_psi::CheckedMachineDeclaration {
            machine: nominal.clone(),
            state: nominal.clone(),
            semantic_declaration: selected,
            parameter_types: Vec::new(),
            callable: selected,
            result_type: selected,
            contract: [0; 32],
            published_may_suspend: false,
            published_may_block: false,
            specialization: Some(terminal_psi::CheckedMachineSpecialization {
                capture: None,
                template_entry: nominal.clone(),
                template: nominal,
                template_callable: selected,
                template_semantic_declaration: selected,
                commitment: [0; 32],
            }),
        });
    // Preserve the old existential rule even for duplicate metadata and an
    // unmatched source owner; neither may re-enable a generated source group.
    let mut duplicate = elaboration.module.machines[1].clone();
    duplicate.contract.declaration = None;
    elaboration.module.machines.push(duplicate);
    elaboration
        .module
        .machines
        .retain(|machine| machine.id != id(3));
    elaboration.module.machines.reverse();
    let expected: Vec<_> = elaboration
        .sources
        .iter()
        .filter(|(machine, _)| {
            !elaboration.module.machines.iter().any(|declared| {
                declared.id == *machine
                    && declared
                        .declaration
                        .as_ref()
                        .is_some_and(|declared| declared.specialization.is_some())
            })
        })
        .map(|(machine, _)| machine)
        .collect();
    let actual: Vec<_> = terminal_check_tables::program::authored_machine_sources(
        &elaboration.module,
        &elaboration.sources,
    )
    .map(|(machine, _)| machine)
    .collect();
    assert_eq!(actual, expected);
    assert_eq!(actual, [id(1), id(3)]);
    let inputs = PreparedMachineInputs::new(
        &elaboration.module,
        &elaboration.sources,
        &elaboration.tables.machines,
        &elaboration.requirements,
        &[],
    );
    assert!(inputs.for_machine(id(2)).unwrap().branches.is_empty());
    for machine in [id(1), id(3)] {
        assert!(std::ptr::eq(
            inputs.for_machine(machine).unwrap().branches,
            elaboration
                .sources
                .get(machine)
                .unwrap()
                .branches
                .as_slice()
        ));
    }
}

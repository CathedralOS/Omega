//! Window plans use retained structural indices, not authored mixed positions.

use checked_trees::{
    CheckedUnitEffectOperationPlan as Operation, CheckedUnitStructuralArgumentSourcePlan as Source,
};
use checked_trees_to_lowered_psi::{TerminalMachineSelection, lower_machine};

#[test]
fn replacement_windows_rejoin_interleaved_structural_parameters() {
    let checked = crate::front_end::checked_program(
        r#"
        data Item [copy] { case Empty; case Ready; }
        data Seed { marker: u32; }
        data Reward { item: Item; sibling: u64; }
        data Factory { marker: u32; }
        machine Factory::choose(&mut self, depth: u32, seed: &mut Seed) -> Item {
            self.marker = depth;
            seed.marker = depth;
            transition depth == 7 { true -> Item::Ready false -> Item::Empty }
        }
        machine Factory::fill(&mut self, depth: u32, seed: &mut Seed, reward: &mut Reward, other: &mut Reward) {
            transition { _ -> install(depth, seed, reward, other) }
            state install(&mut self, depth: u32, seed: &mut Seed, reward: &mut Reward, other: &mut Reward) {
                reward.item = self.choose(depth, seed);
            }
        }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Factory::fill"))
        .expect("scalar formals do not shift a structural window's root");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("exact destination and call result independently verify");
    let plan_index = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .position(|plan| checked.symbols.display_path(plan.machine, "::") == "Factory::fill")
        .expect("named-state field replacement");
    let state_index = checked.facts.flow.terminal_unit_effects.composed_machines[plan_index]
        .states
        .iter()
        .position(|state| {
            state
                .operations
                .iter()
                .any(|operation| matches!(operation, Operation::MoveStructuralField { .. }))
        })
        .expect("replacement state");
    let state =
        &checked.facts.flow.terminal_unit_effects.composed_machines[plan_index].states[state_index];
    let root = state
        .operations
        .iter()
        .find_map(|operation| match operation {
            Operation::MoveStructuralField { source, .. } => source.source_parameter_index(),
            _ => None,
        })
        .expect("window parameter root");
    let retained = &state.structural_parameters[root as usize];
    assert_eq!(
        retained.position, 3,
        "reward's authored position includes depth"
    );
    assert_ne!(
        root, retained.position,
        "the window uses the structural lane"
    );
    let other = state
        .structural_parameters
        .iter()
        .position(|parameter| parameter.position == 4)
        .unwrap() as u32;
    for wrong_root in [0, other, u32::MAX] {
        let mut changed = checked.clone();
        let state = &mut changed.facts.flow.terminal_unit_effects.composed_machines[plan_index]
            .states[state_index];
        for operation in &mut state.operations {
            let place = match operation {
                Operation::MoveStructuralField { source, .. } => source,
                Operation::StoreStructuralField { destination, .. } => destination,
                _ => continue,
            };
            place.source = Source::Parameter {
                parameter_index: wrong_root,
            };
        }
        lower_machine(&changed, TerminalMachineSelection::Name("Factory::fill"))
            .expect_err("a paired window at another root cannot replace the authored destination");
    }
}

//! Call scheduling follows retained formal positions, not receiver syntax.
use checked_trees_to_lowered_psi::{TerminalMachineSelection, lower_machine};

#[test]
fn copied_arguments_keep_positions_with_retained_or_ambient_receivers() {
    for (body, retains_receiver) in [("", false), ("self.sentinel = 31;", true)] {
        let source = format!(
            "data Tag [copy] {{ case First; case Second; }}
             data Receiver {{ sentinel: u64; }}
             data Root {{ receiver: Receiver; tag: Tag; }}
             machine Receiver::accept(&mut self, tag: Tag) {{ {body} }}
             machine Root::exercise(&mut self) {{ self.receiver.accept(self.tag); }}"
        );
        let checked = crate::front_end::checked_program(&source);
        let target = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Receiver::accept")
            .expect("authored callee")
            .symbol;
        let target_index = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .position(|plan| plan.machine == target)
            .expect("ordinary callee");
        let formals =
            &checked.facts.flow.terminal_unit_effects.machines[target_index].structural_parameters;
        assert_eq!(
            formals.iter().any(|formal| formal.is_self),
            retains_receiver
        );
        assert_eq!(formals.last().expect("enum formal").position, 1);
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::exercise"))
            .expect("copied argument uses its retained formal, with either receiver presentation");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("independently verify copied argument custody");

        let mut changed = checked.clone();
        changed.facts.flow.terminal_unit_effects.machines[target_index]
            .structural_parameters
            .last_mut()
            .expect("enum formal")
            .position = 0;
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("Root::exercise")).is_err(),
            "the copied enum cannot exchange its authored position with self"
        );

        let mut changed = checked.clone();
        let call = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .flat_map(|plan| &mut plan.operations)
            .filter_map(checked_trees::CheckedUnitEffectOperationPlan::call_mut)
            .find(|call| call.target.machine == target)
            .expect("source-produced consuming call");
        call.structural_arguments
            .insert(0, call.structural_arguments[0].clone());
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("Root::exercise")).is_err(),
            "an extra argument is not authority to invent an ambient receiver slot"
        );
    }
}

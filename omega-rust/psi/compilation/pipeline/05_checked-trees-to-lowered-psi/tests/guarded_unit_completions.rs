//! A selected Unit exit keeps ordinary source and cleanup custody even when
//! other arms jump to named states. The native companion observes guard order.

use checked_trees::{
    CheckedComposedUnitControlTerminatorPlan, CheckedConditionalReturnArm,
    CheckedControlResultPlan, CheckedGuardedJumpArmPlan, CheckedGuardedJumpFallbackPlan,
};
use checked_trees_to_lowered_psi::{TerminalMachineSelection, lower_machine};
use language_semantics::{PermissionEventKind, PermissionEventSource};

#[test]
fn guarded_unit_completions_replay_source_ordinals_and_cleanup() {
    for middle in [false, true] {
        let (second, fallback) = if middle {
            ("{}", "finish(token, local)")
        } else {
            ("finish(token, local)", "{}")
        };
        let checked = crate::front_end::checked_program(&format!(
            "data Token {{ value: i32; }}
             machine make() -> Token {{ Token {{ value: 42 }} }}
             machine route(token: Token, first: bool, second: bool) {{
                 let local: Token = make();
                 transition {{
                     first -> finish(token, local)
                     second -> {second}
                     _ -> {fallback}
                 }}
                 state finish(token: Token, local: Token) {{}}
             }}"
        ));
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "route")
            .unwrap();
        let source = checked.machine_states(machine)[0].clone();
        let machine_symbol = machine.symbol;
        let plan_index = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter()
            .position(|plan| plan.machine == machine_symbol)
            .expect("mixed jumps and Unit completion retain their ordinary graph");
        let ordinal = if middle { 2 } else { 3 };
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("route"))
            .expect("middle and fallback Unit returns lower with owned cleanup");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("every selected Unit exit preserves its live ownership frontier");
        assert!(lowered.semantic_module.machines.iter().any(|machine| {
            machine.blocks.iter().any(|block| {
                matches!(&block.terminator, terminal_psi::Terminator::ReturnUnit {
                    trivial_affine_discards, ..
                } if trivial_affine_discards.len() == 2)
            })
        }));

        let statements = checked.statement_table.statements(source.statement_nodes);
        let checked_trees::statement::StatementNode::Transition(jump) = &statements[1] else {
            panic!("first authored arm is a jump");
        };
        let jump_target = jump.target;
        for corruption in [
            "ordinal",
            "result",
            "source",
            "local cleanup",
            "parameter cleanup",
        ] {
            let mut changed = checked.clone();
            match corruption {
                "ordinal" => {
                    let plan =
                        &mut changed.facts.flow.terminal_unit_effects.composed_machines[plan_index];
                    let CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, fallback } =
                        &mut plan.states[0].terminator
                    else {
                        panic!("ordered three-arm graph");
                    };
                    let completion = if middle {
                        let CheckedGuardedJumpArmPlan::Return(arm) = &mut arms[1] else {
                            panic!("middle Unit arm");
                        };
                        &mut arm.return_arm
                    } else {
                        let CheckedGuardedJumpFallbackPlan::Return(arm) = fallback else {
                            panic!("Unit fallback");
                        };
                        arm
                    };
                    let CheckedConditionalReturnArm::Unit { statement_ordinal } = completion else {
                        panic!("Unit completion");
                    };
                    assert_eq!(*statement_ordinal, ordinal);
                    *statement_ordinal = 1;
                }
                "result" => {
                    changed.facts.flow.terminal_unit_effects.composed_machines[plan_index].result =
                        CheckedControlResultPlan::Scalar {
                            primitive_type: checked_trees::types::PrimitiveType::Bool,
                        };
                }
                "source" => {
                    let checked_trees::statement::StatementNode::Transition(completion) =
                        &mut changed
                            .typed
                            .statement_table
                            .statements_mut(source.statement_nodes)[ordinal as usize]
                    else {
                        panic!("authored completion");
                    };
                    completion.target = jump_target;
                }
                "local cleanup" | "parameter cleanup" => {
                    let symbol = if corruption == "local cleanup" {
                        let checked_trees::statement::StatementNode::LocalData(local) =
                            &statements[0]
                        else {
                            panic!("owned local");
                        };
                        local.symbol
                    } else {
                        checked.state_parameters(&source)[0].symbol
                    };
                    let (drop, _) = checked
                        .facts
                        .flow
                        .ownership
                        .permissions
                        .iter()
                        .find(|(_, event)| {
                            event.machine_symbol == machine_symbol
                                && event.state_symbol == source.symbol
                                && event.root == facts::PlaceRoot::Symbol(symbol)
                                && event.source == PermissionEventSource::StateExit
                                && event.kind == PermissionEventKind::AffineDrop
                        })
                        .expect("Unit completion retains ordinary owned cleanup");
                    changed
                        .facts
                        .flow
                        .ownership
                        .permissions
                        .get_mut(drop)
                        .source = PermissionEventSource::StateEntry;
                }
                _ => unreachable!(),
            }
            assert!(
                lower_machine(&changed, TerminalMachineSelection::Name("route")).is_err(),
                "middle={middle}, corruption={corruption}"
            );
        }
    }
}

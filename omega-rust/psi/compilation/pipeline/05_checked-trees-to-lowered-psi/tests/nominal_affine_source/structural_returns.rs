//! A conditional generic return transfers the selected owner and cleans the
//! other under its own prerequisite, including across a forwarding call.
use super::{
    AcceptTerminalEffects, AdmissionProfile, TerminalAffineCleanupAction, TerminalExecutionResult,
    Terminator, decode_module, decode_proof_bundle, encode_module, encode_proof_section,
    interpret_terminal_artifact_measured,
};
use checked_trees::{CheckedNormalCleanupAction, CheckedUnitPartialAffineDiscardPlan};
use checked_trees_to_lowered_psi::{TerminalMachineSelection, lower_machine};
use terminal_fuel::FuelChargeSite;
use terminal_interpreter::TerminalStructuralInputs;

fn source(keep_first: bool, forwarded: bool) -> String {
    let forwarding = if forwarded {
        "machine forward<T>(flag: bool, first: T, second: T) -> T {
            choose(flag, first, second)
        }"
    } else {
        ""
    };
    let target = if forwarded { "forward" } else { "choose" };
    format!(
        "data Token {{ ready: bool; }}
        machine Token::drop(owner: &mut Token) requires owner.ready {{}}
        machine choose<T>(keep_first: bool, first: T, second: T) -> T {{
            transition keep_first {{ true -> first false -> second }}
        }}
        {forwarding}
        machine run() -> Token {{
            {target}({keep_first}, Token {{ ready: {} }}, Token {{ ready: {keep_first} }})
        }}",
        !keep_first,
    )
}

#[test]
fn conditional_structural_returns_execute_only_the_unreturned_owners_cleanup() {
    for keep_first in [false, true] {
        for forwarded in [false, true] {
            let checked = crate::front_end::checked_program(&source(keep_first, forwarded));
            let lowered = lower_machine(&checked, TerminalMachineSelection::Name("run"))
                .expect("conditional structural cleanup lowers through the ordinary closure");
            let semantics =
                encode_module(&lowered.semantic_module).expect("semantic artifact encodes");
            let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
                .expect("proof artifact encodes");
            let module = decode_module(&semantics).expect("source-free module decodes");
            let proofs = decode_proof_bundle(&proof).expect("source-free proof decodes");
            terminal_verifier::verify_module(&module, &proofs, &AdmissionProfile::default())
                .expect("selected edge obligations reverify without source");
            let mut hook = None;
            let mut owned_edges = Vec::new();
            for machine in &module.machines {
                for block in &machine.blocks {
                    let Terminator::ReturnStructural {
                        source,
                        cleanup_actions,
                        ..
                    } = &block.terminator
                    else {
                        continue;
                    };
                    if cleanup_actions.is_empty() {
                        continue;
                    }
                    let [TerminalAffineCleanupAction::InvokeNominal(cleanup)] =
                        cleanup_actions.as_slice()
                    else {
                        panic!("each choice disposes exactly its unreturned owner");
                    };
                    assert_ne!(*source, cleanup.place);
                    assert_eq!(cleanup.requirement_obligations.len(), 1);
                    assert!(
                        proofs
                            .evidence
                            .iter()
                            .any(|evidence| evidence.obligation
                                == cleanup.requirement_obligations[0])
                    );
                    if let Some(prior) = hook {
                        assert_eq!(prior, cleanup.cleanup_machine);
                    }
                    hook = Some(cleanup.cleanup_machine);
                    owned_edges.push(cleanup.place);
                }
            }
            assert_eq!(
                owned_edges.len(),
                2,
                "both choices retain their distinct cleanup occurrence"
            );
            assert_ne!(owned_edges[0], owned_edges[1]);
            let hook = module
                .machines
                .iter()
                .find(|machine| Some(machine.id) == hook)
                .expect("selected hook is in the closure");
            let hook_returns = hook
                .blocks
                .iter()
                .map(|block| FuelChargeSite::Edge(block.terminator.edge()))
                .collect::<Vec<_>>();
            let measured = interpret_terminal_artifact_measured(
                &semantics,
                &proof,
                &AdmissionProfile::default(),
                &[],
                TerminalStructuralInputs::default(),
                &mut AcceptTerminalEffects,
            )
            .expect("conditional cleanup executes the encoded artifact");
            assert!(matches!(
                measured.value(),
                TerminalExecutionResult::Structural(_)
            ));
            assert_eq!(
                hook_returns
                    .iter()
                    .filter_map(|site| measured.usage().at(*site))
                    .map(|usage| usage.executions())
                    .sum::<u64>(),
                1,
                "only the selected owner cleanup executes"
            );
            assert!(measured.effects().is_empty());
        }
    }
}

#[test]
fn conditional_structural_cleanup_rejects_changed_source_edge_custody() {
    let checked = crate::front_end::checked_program(&source(true, true));
    lower_machine(&checked, TerminalMachineSelection::Name("run"))
        .expect("baseline source publishes");
    let (machine_index, state_index) = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .enumerate()
        .find_map(|(machine_index, machine)| {
            machine
                .states
                .iter()
                .position(|state| {
                    state
                        .normal_exit_cleanups
                        .iter()
                        .filter(|row| {
                            matches!(
                                row.actions.as_slice(),
                                [CheckedNormalCleanupAction::InvokeNominal(_)]
                            )
                        })
                        .count()
                        == 2
                })
                .map(|state_index| (machine_index, state_index))
        })
        .expect("two independently selected cleanup exits retained");
    for mutation in 0..6 {
        let mut changed = checked.clone();
        let rows = &mut changed.facts.flow.terminal_unit_effects.composed_machines[machine_index]
            .states[state_index]
            .normal_exit_cleanups;
        let first = rows.iter().position(|row| !row.actions.is_empty()).unwrap();
        let second = rows
            .iter()
            .enumerate()
            .find_map(|(index, row)| (index != first && !row.actions.is_empty()).then_some(index))
            .unwrap();
        match mutation {
            0 => {
                rows[first].actions.clear();
            }
            1 => {
                rows.push(rows[first].clone());
            }
            2 => {
                let first_exit = rows[first].exit;
                rows[first].exit = rows[second].exit;
                rows[second].exit = first_exit;
            }
            3 => {
                rows[first].actions[0] = rows[second].actions[0].clone();
            }
            4 => {
                let CheckedNormalCleanupAction::InvokeNominal(cleanup) =
                    rows[first].actions[0].clone()
                else {
                    unreachable!()
                };
                rows[first].actions[0] =
                    CheckedNormalCleanupAction::Discard(CheckedUnitPartialAffineDiscardPlan {
                        source: cleanup.source,
                        path: Vec::new(),
                        type_identity: cleanup.type_identity,
                    });
            }
            5 => {
                rows.remove(first);
            }
            _ => unreachable!(),
        }
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("run")).is_err(),
            "source cleanup mutation {mutation} must reject"
        );
    }
}

#[test]
fn conditional_structural_cleanup_cannot_consume_its_returned_owner() {
    let checked = crate::front_end::checked_program(&source(false, false));
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("run"))
        .expect("baseline source publishes");
    let mut changed = lowered.semantic_module.clone();
    let mut corrupted = false;
    for machine in &mut changed.machines {
        for block in &mut machine.blocks {
            if let Terminator::ReturnStructural {
                source,
                cleanup_actions,
                ..
            } = &mut block.terminator
                && let [TerminalAffineCleanupAction::InvokeNominal(cleanup)] =
                    cleanup_actions.as_mut_slice()
            {
                cleanup.place = *source;
                corrupted = true;
                break;
            }
        }
        if corrupted {
            break;
        }
    }
    assert!(corrupted);
    assert!(
        terminal_verifier::verify_module(
            &changed,
            &lowered.proof_bundle,
            &AdmissionProfile::default()
        )
        .is_err(),
        "source-free replay retains the returned owner's custody"
    );
}

#[test]
fn conditional_cleanup_proof_requires_its_selected_guard() {
    let checked = crate::front_end::checked_program(&source(true, true));
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("run"))
        .expect("baseline source publishes");
    let mut changed = lowered.semantic_module.clone();
    let chooser = changed.machines.iter_mut().find(|machine| {
        machine.blocks.iter().filter(|block| matches!(
            &block.terminator,
            Terminator::ReturnStructural { cleanup_actions, .. }
                if matches!(cleanup_actions.as_slice(), [TerminalAffineCleanupAction::InvokeNominal(_)])
        )).count() == 2
    }).expect("conditional owner has two distinct cleanup returns");
    let branch = chooser
        .blocks
        .iter_mut()
        .find_map(|block| {
            if let Terminator::Conditional {
                when_true,
                when_false,
                ..
            } = &mut block.terminator
            {
                Some((when_true, when_false))
            } else {
                None
            }
        })
        .expect("cleanup choices follow a real conditional edge");
    std::mem::swap(branch.0, branch.1);
    // Both arms still return one owner and dispose the other: custody alone
    // remains valid. The changed edge no longer supplies the premise under
    // which that other owner's ready field is required at invocation entry.
    terminal_verifier::validate_module(&changed)
        .expect("exchanging complete successors preserves structural custody");
    let semantics = encode_module(&changed).expect("changed module encodes");
    let changed = decode_module(&semantics).expect("changed module decodes without source");
    assert!(
        terminal_verifier::verify_module(
            &changed,
            &lowered.proof_bundle,
            &AdmissionProfile::default()
        )
        .is_err(),
        "a cleanup certificate cannot reuse the opposite edge's truth"
    );
}

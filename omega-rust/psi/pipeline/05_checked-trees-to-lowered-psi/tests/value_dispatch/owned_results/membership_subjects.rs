use super::{MEMBERSHIP_TYPES, membership_case, verify_membership_source};
use crate::value_dispatch::{TerminalExecutionResult, TerminalScalarValue};

/// A `subject in Type::Case` dispatch guard is a tag observation, not an
/// ownership transfer: borrowed, copy, affine-parameter, `self`, and affine
/// local subjects each rejoin the authored membership while the owned
/// candidates keep exact residual custody.
#[test]
fn owned_match_membership_subject_observes_without_transferring_it() {
    for parameter in ["view: &Kind", "view: Kind"] {
        verify_membership_source(&format!(
            "{MEMBERSHIP_TYPES}
            machine choose({parameter}, left: Choice, right: Choice) -> bool {{
                let result: Choice = match view in Kind::Missing {{ true -> left, false -> right }};
                result in Choice::Full
            }}"
        ));
    }
    verify_membership_source(&format!(
        "{MEMBERSHIP_TYPES}
        machine Kind::choose(&self, left: Choice, right: Choice) -> bool {{
            let result: Choice = match self in Kind::Missing {{ true -> left, false -> right }};
            result in Choice::Full
        }}"
    ));
}

/// The affine local subject is only observed by the membership guard, so it
/// rides the result join as a transported owner while the parameter candidates
/// hold residual slots. The return cleanup keeps the join's
/// reverse-declaration order: selected result, newer transported owner, then
/// the surviving candidate's residual slot.
#[test]
fn owned_match_membership_subject_preserves_join_cleanup_order() {
    let source = format!(
        "{MEMBERSHIP_TYPES}
        machine choose(flag: bool, left: Choice, right: Choice) -> bool {{
            let kind: Kind = Kind::Missing;
            let result: Choice = match kind in Kind::Missing {{ true -> left, false -> right }};
            result in Choice::Full
        }}"
    );
    let (_, lowered) = verify_membership_source(&source);
    let machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry");
    let join = machine
        .blocks
        .iter()
        .find(|block| !block.structural_parameters.is_empty())
        .expect("membership result join");
    assert_eq!(join.structural_parameters.len(), 3);
    let cleanup_actions = machine
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            terminal_psi::Terminator::Return {
                cleanup_actions, ..
            } => Some(cleanup_actions.as_slice()),
            _ => None,
        })
        .expect("scalar return cleanup");
    assert_eq!(cleanup_actions.len(), 3, "{cleanup_actions:?}");
    let discard_places = cleanup_actions
        .iter()
        .map(|action| match action {
            terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place) => *place,
            _ => panic!("membership cleanup is plain root discards: {action:?}"),
        })
        .collect::<Vec<_>>();
    // Reverse join-declaration order: the result slot, then the residual and
    // transported-owner slots in reverse declaration position.
    let mut declared = join
        .structural_parameters
        .iter()
        .map(|parameter| (parameter.position, parameter.place))
        .collect::<Vec<_>>();
    declared.sort();
    let expected = declared
        .iter()
        .rev()
        .map(|(_, place)| *place)
        .collect::<Vec<_>>();
    assert_eq!(discard_places, expected);
    // Swapping the residual and transported-owner discards is the exact
    // ordering regression this case witnessed.
    let mut changed = lowered.semantic_module.clone();
    let machine = changed
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed.entry)
        .unwrap();
    let cleanup_actions = machine
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            terminal_psi::Terminator::Return {
                cleanup_actions, ..
            } => Some(cleanup_actions),
            _ => None,
        })
        .expect("scalar return cleanup");
    cleanup_actions.swap(1, 2);
    assert!(
        terminal_verifier::verify_module(
            &changed,
            &lowered.proof_bundle,
            &crate::value_dispatch::AdmissionProfile::default()
        )
        .is_err(),
        "swapped residual/owner cleanup order must reject"
    );
}

/// The observed subjects stay executable: an owned parameter subject supplies
/// its tag through the host case bindings and an affine local subject keeps
/// its produced tag; both select the exact owned candidate.
#[test]
fn owned_match_membership_subject_executes_the_selected_arm() {
    let parameter_source = format!(
        "{MEMBERSHIP_TYPES}
        machine choose(view: Kind, left: Choice, right: Choice) -> bool {{
            let result: Choice = match view in Kind::Missing {{ true -> left, false -> right }};
            result in Choice::Full
        }}"
    );
    for (subject_case, expected) in [("Missing", true), ("Other", false)] {
        let (_, lowered) = verify_membership_source(&parameter_source);
        let semantic_bytes =
            terminal_codec::encode_module(&lowered.semantic_module).expect("encode semantics");
        let proof_bytes =
            terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
                .expect("encode proof");
        let module = &lowered.semantic_module;
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .expect("entry");
        let arguments = entry
            .structural_parameters
            .iter()
            .enumerate()
            .map(
                |(index, parameter)| terminal_interpreter::TerminalStructuralValue {
                    opaque_identity: 0x4130 + index as u64,
                    structural_type: parameter.structural_type,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                },
            )
            .collect::<Vec<_>>();
        let cases = [
            terminal_interpreter::TerminalStructuralCaseValue {
                argument_index: 0,
                path: Vec::new(),
                case: membership_case(module, "Kind", subject_case),
            },
            terminal_interpreter::TerminalStructuralCaseValue {
                argument_index: 1,
                path: Vec::new(),
                case: membership_case(module, "Choice", "Full"),
            },
            terminal_interpreter::TerminalStructuralCaseValue {
                argument_index: 2,
                path: Vec::new(),
                case: membership_case(module, "Choice", "Empty"),
            },
        ];
        let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
            &semantic_bytes,
            &proof_bytes,
            &crate::value_dispatch::AdmissionProfile::default(),
            &[],
            terminal_interpreter::TerminalStructuralInputs {
                arguments: &arguments,
                cases: &cases,
                ..Default::default()
            },
        )
        .expect("membership subject execution starts");
        let status = execution
            .resume(
                &mut terminal_fuel::TerminalFuelMeter::unbounded(),
                &mut terminal_interpreter::AcceptTerminalEffects,
            )
            .expect("membership subject execution completes");
        assert_eq!(
            status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
            ),
            "subject_case={subject_case}"
        );
    }
}

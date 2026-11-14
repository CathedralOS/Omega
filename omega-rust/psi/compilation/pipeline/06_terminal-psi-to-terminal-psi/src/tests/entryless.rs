//! Source validation claims the whole subject without inventing a program entry.

use super::boundary_calls::typed;
use super::*;

pub(super) fn judged(text: &str, select_first: bool) -> RouteVerdict {
    let typed = typed(text);
    let selected = if select_first {
        vec![typed.machines()[0].symbol]
    } else {
        Vec::new()
    };
    let mut verdicts = crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                entries: &[],
                subjects: &[None],
                purpose: source::DependencyScope::Product,
                selected: &selected,
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .program,
    );
    assert_eq!(verdicts.len(), 1);
    verdicts.remove(0).unwrap()
}

#[test]
fn a_subject_without_entries_checks_every_machine_without_picking_one_as_entry() {
    let result = judged(
        "machine helper(value: u32) -> u32 { value - 1 }\n\
         machine caller() -> u32 { helper(1) }\n",
        false,
    );
    let RouteVerdict::Accepted(accepted) = result else {
        panic!("{result:?}")
    };
    let elaboration = &accepted.elaboration;
    assert_eq!(elaboration.module.entry, None);
    assert_eq!(elaboration.module.machines.len(), 2);
    // Machine 1 is an ordinary private helper. Making it an invented entry
    // would forbid the closed-subject input inference this body needs.
    assert!(elaboration.private_inputs.contains(&id(1)));
}

#[test]
fn an_unused_invalid_subject_machine_is_still_rejected_without_entries() {
    let result = judged("machine unused(value: u32) -> u32 { value - 1 }\n", false);
    let RouteVerdict::Rejected { unproved, .. } = result else {
        panic!("{result:?}")
    };
    assert!(
        unproved
            .iter()
            .any(|row| matches!(row.kind, ObligationKind::OperationSafety { .. }))
    );
}

#[test]
fn a_selected_machine_keeps_its_input_boundary_without_an_executable_entry() {
    let result = judged(
        "machine backing(value: u32) -> u32 { value - 1 }\n\
         machine caller() -> u32 { backing(1) }\n",
        true,
    );
    let RouteVerdict::Rejected { unproved, .. } = result else {
        panic!("{result:?}")
    };
    assert!(
        unproved
            .iter()
            .any(|row| matches!(row.kind, ObligationKind::OperationSafety { .. }))
    );
}

#[test]
fn a_data_only_subject_needs_no_placeholder_machine() {
    let result = judged("data Payload { value: u32 }\n", false);
    let RouteVerdict::Accepted(accepted) = result else {
        panic!("{result:?}")
    };
    assert_eq!(accepted.elaboration.module.entry, None);
    assert!(accepted.elaboration.module.machines.is_empty());
    assert_eq!(accepted.elaboration.module.structural_types.len(), 1);
}

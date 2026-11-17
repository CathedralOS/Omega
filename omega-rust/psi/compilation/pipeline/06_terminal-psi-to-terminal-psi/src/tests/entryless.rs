//! Source validation claims the whole subject without inventing a program entry.

use super::boundary_calls::typed;
use super::*;

pub(super) fn judged(text: &str, select_first: bool) -> RouteVerdict {
    let typed = typed(text);
    judged_typed(&typed, select_first)
}

pub(super) fn judged_typed(typed: &typed_trees::TypedTrees, select_first: bool) -> RouteVerdict {
    judged_typed_result(typed, select_first).unwrap()
}

pub(super) fn judged_typed_result(
    typed: &typed_trees::TypedTrees,
    select_first: bool,
) -> Result<RouteVerdict, crate::CompilationRefusal> {
    let mut verdicts = judged_parts(typed, select_first);
    // The first verdict is the closure's. Any other is a further refusal of
    // the same reading, never an acceptance.
    assert!(
        verdicts
            .iter()
            .skip(1)
            .all(|verdict| !matches!(verdict, Ok(RouteVerdict::Accepted(_)))),
        "{verdicts:?}"
    );
    verdicts.remove(0)
}

fn judged_parts(
    typed: &typed_trees::TypedTrees,
    select_first: bool,
) -> Vec<Result<RouteVerdict, crate::CompilationRefusal>> {
    let selected = if select_first {
        vec![typed.machines()[0].symbol]
    } else {
        Vec::new()
    };
    crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            typed,
            typed_trees_to_terminal_psi::Compilation {
                calling: None,
                providers: None,

                declaration_metadata: &[],
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
    )
}

#[test]
fn whole_subject_acceptance_requires_every_generic_check_form() {
    let typed = typed("machine identity<T>(value: T) -> T { move value }\n");
    let parts = judged_parts(&typed, false);
    assert!(
        parts.len() > 1,
        "the generic form must be judged: {parts:?}"
    );
    assert!(
        parts
            .iter()
            .all(|part| matches!(part, Ok(RouteVerdict::Accepted(_)))),
        "{parts:?}"
    );
    let result = judged_typed(&typed, false);
    assert!(matches!(result, RouteVerdict::Accepted(_)), "{result:?}");
}

#[test]
fn a_later_generic_body_refusal_prevents_whole_subject_acceptance() {
    let typed = typed(
        "data Envelope<T> { item: T; }\n\
         machine wrap<T>(value: T) -> Envelope<T> { Envelope { item: move value } }\n",
    );
    let parts = judged_parts(&typed, false);
    assert!(
        matches!(parts.first(), Some(Ok(RouteVerdict::Accepted(_)))),
        "{parts:?}"
    );
    assert!(
        parts
            .iter()
            .skip(1)
            .any(|part| matches!(part, Ok(RouteVerdict::FallBack(_)) | Err(_))),
        "the open generic body must expose its refusal: {parts:?}"
    );
    let result = judged_typed_result(&typed, false);
    assert!(
        !matches!(result, Ok(RouteVerdict::Accepted(_))),
        "the accepted closure must not hide the generic-body refusal: {result:?}"
    );
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

#[test]
fn capacity_guards_reach_the_original_addition_operands() {
    let source = r#"
machine cap(length: u64, position: u64) -> u64
requires position < length {
    transition length - position > 4294967295 {
        true -> receive(length, position + 4294967295)
        false -> position
    }
}
machine advance(length: u64, position: u64, count: i64) -> u64
requires position <= length, count >= 0 {
    let written: u64 = count as u64;
    transition written <= length - position {
        true -> receive(length, position + written)
        false -> position
    }
}
machine receive(length: u64, next: u64) -> u64
requires next <= length { next }
"#;
    let accepted = judged(source, false);
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );

    let different_literal = source.replace("position + 4294967295", "position + 4294967297");
    let rejected = judged(&different_literal, false);
    let RouteVerdict::Rejected { unproved, .. } = rejected else {
        panic!("{rejected:?}");
    };
    assert!(
        unproved
            .iter()
            .any(|row| matches!(row.kind, ObligationKind::OperationSafety { .. }))
    );
}

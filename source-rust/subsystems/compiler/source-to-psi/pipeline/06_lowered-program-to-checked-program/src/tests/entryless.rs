//! Source validation claims the whole subject without inventing a program entry.

use super::boundary_calls::typed;
use super::*;

pub(super) fn judged(text: &str, select_first: bool) -> RouteVerdict {
    let typed = typed(text);
    judged_typed(&typed, select_first)
}

pub(super) fn judged_typed(
    typed: &crate::source_state::TypedTrees,
    select_first: bool,
) -> RouteVerdict {
    judged_typed_result(typed, select_first).unwrap()
}

pub(super) fn judged_typed_result(
    typed: &crate::source_state::TypedTrees,
    select_first: bool,
) -> Result<RouteVerdict, crate::checking::CompilationRefusal> {
    whole_subject_result(judged_parts(typed, select_first))
}

/// A subject passes only if its closure, generic forms and witnesses all pass.
/// Keep the first failure in judgment order. Only an entirely accepted batch
/// returns the original closure, whose tables callers may inspect.
pub(super) fn whole_subject_result(
    mut verdicts: Vec<Result<RouteVerdict, crate::checking::CompilationRefusal>>,
) -> Result<RouteVerdict, crate::checking::CompilationRefusal> {
    assert!(
        !verdicts.is_empty(),
        "a subject must have a closure judgment"
    );
    let first_failure = verdicts
        .iter()
        .position(|verdict| !matches!(verdict, Ok(RouteVerdict::Accepted(_))));
    verdicts.remove(first_failure.unwrap_or(0))
}

fn judged_parts(
    typed: &crate::source_state::TypedTrees,
    select_first: bool,
) -> Vec<Result<RouteVerdict, crate::checking::CompilationRefusal>> {
    let selected = if select_first {
        vec![typed.machines()[0].symbol]
    } else {
        Vec::new()
    };
    crate::checking::judge_compilation(
        crate::lowering::prepare_compilation(
            typed,
            crate::lowering::Compilation {
                calling: None,
                providers: None,

                declaration_metadata: &[],
                entries: &[],
                subjects: &[None],
                purpose: source_provenance::DependencyScope::Product,
                selected: &selected,
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .expect("fixture must lower to checker input")
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
    let RouteVerdict::Accepted(closure) = result else {
        panic!("{result:?}");
    };
    assert!(closure.elaboration.module.machines.is_empty());
}

#[test]
fn a_later_generic_guarantee_rejection_prevents_whole_subject_acceptance() {
    let source = "machine unchanged<T>(value: u32) -> u32 \
                  ensures result == value crashes none { value }\n";
    let accepted = judged(source, false);
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );

    let typed = typed(&source.replace("result == value", "result != value"));
    let parts = judged_parts(&typed, false);
    assert!(
        matches!(parts.first(), Some(Ok(RouteVerdict::Accepted(_)))),
        "{parts:?}"
    );
    assert!(
        parts
            .iter()
            .skip(1)
            .any(|part| matches!(part, Ok(RouteVerdict::Rejected { unproved, .. })
                if unproved.iter().any(|row| matches!(row.kind, ObligationKind::NormalGuarantee { clause: 0, .. })
                    && row.cause == UnprovedCause::NoDerivation))),
        "the generic body must owe its false guarantee: {parts:?}"
    );
    let result = judged_typed_result(&typed, false);
    assert!(
        matches!(result, Ok(RouteVerdict::Rejected { ref unproved, .. })
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::NormalGuarantee { clause: 0, .. })
                && row.cause == UnprovedCause::NoDerivation)),
        "the accepted closure must not hide the generic-body rejection: {result:?}"
    );
}

#[test]
#[should_panic(expected = "a subject must have a closure judgment")]
fn a_subject_with_no_judgments_cannot_be_accepted() {
    let _ = whole_subject_result(Vec::new());
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
    assert_eq!(accepted.elaboration.module.types.structural_types.len(), 1);
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

//! Non-return is a callable fact: an abstract requirement does not expose
//! the stronger private outcome of the body chosen to implement it.

use super::build_purpose::judge_build_all;
use super::*;

fn conformance(inline: bool, required: &str, supplied: &str, caller: &str) -> String {
    let implementation = if inline {
        format!(
            "Stops satisfies Answer {{ machine answer() -> u32 crashes Trap {supplied} {{ crash Trap; }} }}"
        )
    } else {
        format!(
            "machine stopped() -> u32 satisfies Answer::answer crashes Trap {supplied} {{ crash Trap; }}"
        )
    };
    let target = if inline { "Stops::answer" } else { "stopped" };
    format!(
        "trait Answer {{ machine answer() -> u32 crashes Trap {required}; }}\n\
         {implementation}\n\
         machine build_entry() crashes Trap {{ let value: u32 = {target}(); {caller} }}"
    )
}

/// The requirement permits a normal return with any u32. Neither the
/// implementation's impossible postcondition nor its private crash body
/// removes the caller's overflow obligation. Both inline and explicitly
/// declared conformance take the same published-contract route.
#[test]
fn a_returning_requirement_hides_its_members_private_nonreturn() {
    for inline in [false, true] {
        for supplied in ["", "ensures false"] {
            let verdicts = judge_build_all(&conformance(
                inline,
                "",
                supplied,
                "let next: u32 = value + 1;",
            ));
            let [Ok(RouteVerdict::Rejected { unproved, .. }), witnesses @ ..] = &verdicts[..]
            else {
                panic!(
                    "the returning requirement leaves an unsafe addition (inline={inline}, supplied={supplied:?})"
                );
            };
            assert!(
                unproved
                    .iter()
                    .any(|row| matches!(row.kind, ObligationKind::OperationSafety { .. }))
            );
            assert!(
                witnesses
                    .iter()
                    .all(|verdict| matches!(verdict, Ok(RouteVerdict::Accepted(_))))
            );
            assert!(
                witnesses.iter().any(|verdict| {
                    let Ok(RouteVerdict::Accepted(accepted)) = verdict else {
                        return false;
                    };
                    accepted.elaboration.module.machines.iter().any(|machine| {
                        // This member is the program's only authored crash.
                        // A bodyless schema form cannot stand in for it.
                        machine.blocks.iter().any(|block| {
                            matches!(block.terminator, terminal_psi::Terminator::Crash { .. })
                        })
                    })
                }),
                "the actual member body is judged independently"
            );
        }
    }
}

/// Omitting the requirement's crash clause defers that axis to the chosen
/// body. Hiding its non-return must still retain the actual crash route.
#[test]
fn a_hidden_body_still_contributes_its_deferred_crash_route() {
    let verdicts = judge_build_all(
        "trait Answer { machine answer() -> u32; }\n\
         Stops satisfies Answer { machine answer() -> u32 { crash Trap; } }\n\
         machine build_entry() crashes none { let value: u32 = Stops::answer(); }",
    );
    let [Ok(RouteVerdict::Rejected { unproved, .. }), witnesses @ ..] = &verdicts[..] else {
        panic!("the member's deferred crash still needs caller coverage");
    };
    assert!(
        unproved
            .iter()
            .any(|row| matches!(row.kind, ObligationKind::CrashCoverage { .. }))
    );
    assert!(
        witnesses
            .iter()
            .all(|verdict| matches!(verdict, Ok(RouteVerdict::Accepted(_))))
    );
}

/// A public body is not a published no-return promise. Its private crash
/// must not remove the operation checked after an ordinary call either.
#[test]
fn a_public_body_does_not_silently_publish_nonreturn() {
    let verdicts = judge_build_all(
        "public machine stop() -> u32 crashes Trap { crash Trap; }\n\
         machine build_entry() crashes Trap {\n\
             let value: u32 = stop(); let next: u32 = value + 1;\n\
         }",
    );
    let [Ok(RouteVerdict::Rejected { unproved, .. })] = &verdicts[..] else {
        panic!("the public contract permits an unbounded returned value");
    };
    assert!(
        unproved
            .iter()
            .any(|row| matches!(row.kind, ObligationKind::OperationSafety { .. }))
    );
}

/// An ordinary private helper in the same subject still contributes its
/// checked body outcome. This is the local-summary permission, not a fact
/// exported through an abstract contract.
#[test]
fn a_local_private_nonreturn_still_ends_the_callers_path() {
    let verdicts = judge_build_all(
        "machine stop() -> u32 crashes Trap { crash Trap; }\n\
         machine build_entry() crashes Trap {\n\
             let value: u32 = stop(); let next: u32 = value + 1;\n\
         }",
    );
    let [Ok(RouteVerdict::Accepted(accepted))] = &verdicts[..] else {
        panic!("the local checked body excludes the addition: {verdicts:?}");
    };
    assert_eq!(accepted.elaboration.unreachable_statements().count(), 1);
    accepted.publish_invocation().unwrap();
}

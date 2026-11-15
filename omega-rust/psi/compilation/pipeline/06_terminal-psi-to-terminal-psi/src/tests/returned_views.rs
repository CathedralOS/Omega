//! A view a boundary returned, handed on by the machine that called it.
//! Read from source, as the Build prelude's `RequiredOutput::path` is: a
//! one-line machine around `BuildFilesystemOperations::required_path`
//! (`samples/cli/facts/required_output_path`).
//!
//! A boundary requirement has no body, and its declaration says nothing of
//! what backs a view it returns (foreign_storage.md, "Provider-owned views
//! and completion": the referent "may be provider-owned storage outside
//! every input object", and the compiler does not "infer spatial containment
//! from an input's lifetime"). So the machine that returns such a view does
//! not establish its origin. Its callers still have to be judged: they keep
//! everything they passed in live for as long as the returned view is used
//! (lifetimes.md, "Carried-loan transport": an unresolved selection
//! "conservatively retains all possible sources").

use super::*;

/// What the checker says of the Product `text`, judged from its `entry`:
/// `accepted`, `rejected` with its conflicts, or the refusal's text.
fn verdict(text: &str) -> String {
    let typed = super::boundary_calls::typed(text);
    let entry = super::boundary_calls::entry(&typed);
    let elaborated = typed_trees_to_terminal_psi::elaborate_for(
        &typed,
        typed_trees_to_terminal_psi::Compilation {
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
            calling: None,
            providers: None,
        },
        terminal_codec::TerminalPublication::Invocation,
    );
    match crate::judge(elaborated) {
        Ok(RouteVerdict::Accepted(_)) => "accepted".to_owned(),
        Ok(RouteVerdict::FallBack(refusal)) | Err(refusal) => refusal.detail,
        Ok(RouteVerdict::Rejected { conflicts, .. }) => {
            format!("rejected: {} conflicts", conflicts.len())
        }
        Ok(other) => panic!("{other:?}"),
    }
}

/// A machine that forms a view of its own array and also calls a machine
/// returning a view it got from a boundary. The array view makes the checker
/// ask for the machine's reference flow, which needs a summary of every
/// callee, `path` included.
#[test]
fn a_view_a_boundary_returned_is_handed_on_by_a_machine() {
    assert_eq!(
        verdict(
            r#"
data Token { id: u64; }
boundary trait Fs { machine required_path(output: &Token) -> &[u8] reaches none crashes none; }
machine path(output: &Token) -> &[u8] reaches Fs invokes Fs { Fs::required_path(output) }
machine see(bytes: &[u8]) { }
machine entry() reaches Fs invokes Fs {
    let name: [u8; 4] = [1, 2, 3, 4];
    let token: Token = Token { id: 1 };
    see(&name[..]);
    see(path(&token));
}
"#
        ),
        "accepted"
    );
}

/// The twin: nothing says the returned view is not the one passed in, so the
/// array it views stays borrowed while the result is used. Writing the array
/// before the result's last use is a conflict; after it, the array is the
/// machine's again.
#[test]
fn a_view_of_unknown_origin_keeps_what_the_call_was_given() {
    let program = |before: &str, after: &str| {
        format!(
            r#"
boundary trait Fs {{ machine echo(bytes: &[u8]) -> &[u8] reaches none crashes none; }}
machine pass(bytes: &[u8]) -> &[u8] reaches Fs invokes Fs {{ Fs::echo(bytes) }}
machine see(bytes: &[u8]) {{ }}
machine entry() reaches Fs invokes Fs {{
    let mut name: [u8; 4] = [1, 2, 3, 4];
    let rest: &[u8] = pass(&name[..]);
    {before}
    see(rest);
    {after}
}}
"#
        )
    };
    let write = "name[0] = 9;";
    assert_eq!(verdict(&program("", write)), "accepted");
    assert_eq!(verdict(&program(write, "")), "rejected: 1 conflicts");
}

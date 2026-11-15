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
//! not establish its spatial origin. Its callers still have to be judged: they keep
//! everything they passed in live for as long as the returned view is used
//! (lifetimes.md, "Carried-loan transport": an unresolved selection
//! "conservatively retains all possible sources").

use super::*;

fn elaborate(text: &str) -> Checked<typed_trees_to_terminal_psi::Elaboration> {
    let typed = super::boundary_calls::typed(text);
    let entry = super::boundary_calls::entry(&typed);
    typed_trees_to_terminal_psi::elaborate_for(
        &typed,
        typed_trees_to_terminal_psi::Compilation {
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
}

/// What the checker says of the Product `text`, judged from its `entry`:
/// `accepted`, `rejected` with its conflicts, or the refusal's text.
fn verdict(text: &str) -> String {
    match crate::judge(elaborate(text)) {
        Ok(RouteVerdict::Accepted(accepted)) => {
            assert!(
                accepted
                    .elaboration
                    .tables
                    .machines
                    .iter()
                    .any(|(_, tables)| {
                        tables.obligations.iter().any(|(_, row)| {
                            matches!(
                                row.kind,
                                ObligationKind::Loan {
                                    lender: terminal_check_tables::obligations::Lender::Provider,
                                    ..
                                }
                            )
                        })
                    }),
                "the returned view keeps provider permission, not owned or input storage"
            );
            "accepted".to_owned()
        }
        Ok(RouteVerdict::FallBack(refusal)) | Err(refusal) => refusal.detail,
        Ok(RouteVerdict::Rejected { conflicts, .. }) => {
            format!("rejected: {} conflicts", conflicts.len())
        }
        Ok(other) => panic!("{other:?}"),
    }
}

const RETURNED_VIEW: &str = r#"
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
"#;

/// The local array view demands reference flow for every callee, including
/// the helper that hands on the boundary's returned view.
#[test]
fn a_view_a_boundary_returned_is_handed_on_by_a_machine() {
    assert_eq!(verdict(RETURNED_VIEW), "accepted");
}

#[test]
fn a_provider_view_permission_cannot_be_claimed_as_owned_storage() {
    use terminal_check_tables::obligations::Lender;
    let input = elaborate(RETURNED_VIEW).unwrap();
    assert!(matches!(
        crate::judge(Ok(input.clone())),
        Ok(RouteVerdict::Accepted(_))
    ));
    let mut changed = input;
    let (machine, obligation) = changed
        .tables
        .machines
        .iter()
        .find_map(|(machine, tables)| {
            tables.obligations.iter().find_map(|(obligation, row)| {
                matches!(
                    row.kind,
                    ObligationKind::Loan {
                        lender: Lender::Provider,
                        ..
                    }
                )
                .then_some((machine, obligation))
            })
        })
        .expect("the helper lends the provider's returned view");
    let row = changed
        .tables
        .machines
        .get_mut(machine)
        .unwrap()
        .obligations
        .get_mut(obligation)
        .unwrap();
    let ObligationKind::Loan { lender, .. } = &mut row.kind else {
        panic!("the selected obligation is a loan");
    };
    *lender = Lender::Owner;
    assert!(
        matches!(crate::judge(Ok(changed)),
        Err(Refusal { reason: RefusalReason::ForeignIdentity, detail, .. })
        if detail == "reborrow names a different immediate parent occurrence"),
        "a provider result cannot claim ownership of its payload"
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

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

/// Public visibility does not hide an exact checked body from its own unit.
/// The inferred outcome changes this local continuation, not the public API.
#[test]
fn a_local_public_nonreturn_still_ends_the_callers_path() {
    let verdicts = judge_build_all(
        "public machine stop() -> u32 crashes Trap { crash Trap; }\n\
         machine build_entry() crashes Trap {\n\
             let value: u32 = stop(); let next: u32 = value + 1;\n\
         }",
    );
    let [Ok(RouteVerdict::Accepted(accepted))] = &verdicts[..] else {
        panic!("the exact local body excludes the addition: {verdicts:?}");
    };
    assert_eq!(accepted.elaboration.unreachable_statements().count(), 1);
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(1_000))
        .unwrap();
    assert!(
        matches!(status, terminal_interpreter::TerminalExecutionStatus::Crashed(ref crash) if crash.cause == semantic_vocabulary::CrashCause::Trap),
        "{status:?}"
    );
}

/// The same public body in a dependency remains an ordinary returning API.
/// Importing its source does not let the client skip the unsafe addition.
#[test]
fn a_dependency_body_does_not_silently_publish_nonreturn() {
    let mut sources = source::SourceMap::default();
    let mut syntax = None;
    let mut source_ids = Vec::with_capacity(2);
    let client_package = semantic_vocabulary::PackageKeyIdentity::from_digest([2; 32]).unwrap();
    for (path, root, text, digest) in [
        (
            "library/api.omg",
            "library",
            "public machine stop() -> u32 crashes Trap { crash Trap; }",
            [1; 32],
        ),
        (
            "client/main.omg",
            "client",
            "machine entry() crashes Trap { let value: u32 = stop(); let next: u32 = value + 1; }",
            [2; 32],
        ),
    ] {
        let source = sources
            .add_checked_instance(
                path.into(),
                text,
                root.into(),
                Some(semantic_vocabulary::PackageKeyIdentity::from_digest(digest).unwrap()),
                source::SourceOrigin::User,
                source::SourceResolutionStratum::Base,
                source::DependencyScope::Product,
            )
            .source_id;
        source_ids.push(source);
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        match &mut syntax {
            Some(syntax) => {
                tokens_to_syntax_trees::parse(syntax, source, &tokens).unwrap();
            }
            None => {
                syntax = Some(
                    tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap(),
                )
            }
        }
    }
    let mut request =
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax.unwrap());
    request.sources = Some(std::sync::Arc::new(sources));
    request
        .top_level_bindings
        .push(symbols::SourceScopedTopLevelBinding::new(
            source_ids[1],
            source_ids[0],
            "stop",
        ));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let entry = super::build_purpose::symbol(&typed, "entry");
    let verdicts = crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            super::build_purpose::roster(
                &[entry],
                &[Some(client_package)],
                source::DependencyScope::Product,
            ),
            terminal_codec::TerminalPublication::Invocation,
        )
        .program,
    );
    let [Ok(RouteVerdict::Rejected { unproved, .. })] = &verdicts[..] else {
        panic!("the dependency contract permits a returned value: {verdicts:?}");
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

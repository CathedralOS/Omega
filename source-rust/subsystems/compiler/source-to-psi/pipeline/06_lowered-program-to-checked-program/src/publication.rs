//! Only a completely accepted compilation can issue its source for publication.
use super::verdict::{AcceptedClosure, CompilationRefusal, RouteVerdict, judge_compilation};
use diagnostics::Diagnostic;
use terminal_check_tables::compilation::CompilationForPublication;

/// The input type identifies the lowering operation that supplied both the
/// terminal compilation and the source to retain beside the checked closures.
pub struct CheckedPublication<Input: CompilationForPublication> {
    source: Input::Source,
    accepted: AcceptedCompilation,
}

impl<Input: CompilationForPublication> CheckedPublication<Input> {
    pub fn into_source_and_closures(
        self,
    ) -> (
        Input::Source,
        Box<AcceptedClosure>,
        Vec<Box<AcceptedClosure>>,
    ) {
        (self.source, self.accepted.closure, self.accepted.additional)
    }
}

pub fn judge_publication<Input: CompilationForPublication>(
    input: Input,
) -> Result<CheckedPublication<Input>, Vec<Diagnostic>> {
    let (program, source) = input.into_program_and_source();
    let accepted = accept_compilation(judge_compilation(program))?;
    Ok(CheckedPublication { source, accepted })
}

/// All semantic verdicts accepted. Additional parts supply only their
/// diagnostics after the source correspondence has been checked.
#[derive(Debug)]
struct AcceptedCompilation {
    closure: Box<AcceptedClosure>,
    additional: Vec<Box<AcceptedClosure>>,
}

fn accept_compilation(
    verdicts: Vec<Result<RouteVerdict, CompilationRefusal>>,
) -> Result<AcceptedCompilation, Vec<Diagnostic>> {
    let mut first = None;
    let mut additional = Vec::new();
    let mut diagnostics = Vec::new();
    for (position, verdict) in verdicts.into_iter().enumerate() {
        match verdict {
            Ok(RouteVerdict::Accepted(accepted)) => {
                if position == 0 {
                    first = Some(accepted);
                } else {
                    additional.push(accepted);
                }
            }
            Ok(RouteVerdict::Rejected {
                formations,
                violations,
                unproved,
                unestablished,
                conflicts,
                exceeded,
            }) => {
                let before = diagnostics.len();
                diagnostics.extend(formations.iter().map(|entry| entry.reject_diagnostic()));
                diagnostics.extend(violations.iter().map(|entry| entry.reject_diagnostic()));
                for entry in unproved.iter().filter(|entry| !entry.cause.is_incomplete()) {
                    diagnostics.push(entry.reject_diagnostic());
                    diagnostics.extend(entry.opening_diagnostic());
                }
                diagnostics.extend(unestablished.iter().map(|entry| entry.reject_diagnostic()));
                diagnostics.extend(conflicts.iter().map(|entry| entry.reject_diagnostic()));
                diagnostics.extend(exceeded.iter().map(|entry| entry.reject_diagnostic()));
                if before == diagnostics.len() {
                    diagnostics.push(route_defect("a rejection names no failure"));
                }
            }
            Ok(RouteVerdict::Incomplete(unproved)) => {
                if unproved.is_empty() {
                    diagnostics.push(route_defect("an incomplete verdict names no goal"));
                }
                diagnostics.extend(unproved.iter().map(|entry| {
                    let mut diagnostic = entry.reject_diagnostic();
                    diagnostic.message = format!(
                        "not supported yet: {}; the checker cannot prove a goal of this form yet, so the program is refused, with no claim that it is wrong",
                        diagnostic.message
                    );
                    diagnostic
                }));
            }
            Ok(RouteVerdict::FallBack(refused)) => {
                let mut diagnostic = Diagnostic::error(format!(
                    "not supported yet: {}; the checker does not support this construct yet, so the program is refused, with no claim that it is wrong",
                    refused.detail
                ));
                diagnostic.source_span = refused.source_span;
                diagnostics.push(diagnostic);
            }
            Err(defect) => diagnostics.push(route_defect(&format!("{defect:?}"))),
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let closure = first.ok_or_else(|| vec![route_defect("no accepted compilation root")])?;
    // Classification requests no warnings. The source owner finishes its
    // correspondence checks before consuming these transient parts.
    Ok(AcceptedCompilation {
        closure,
        additional,
    })
}

fn route_defect(detail: &str) -> Diagnostic {
    Diagnostic::error(format!(
        "not supported yet: the checker could not judge this program ({detail}); this is a defect of the checker, and the program is refused rather than accepted without a verdict"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lowering::Compilation;
    use std::sync::Arc;

    fn elaboration(
        publication: terminal_codec::TerminalPublication,
    ) -> crate::lowering::Elaboration {
        const TEXT: &str = "machine entry() crashes none {}";
        let package = semantic_vocabulary::PackageKeyIdentity::from_digest([0xa7; 32]).unwrap();
        let mut sources = crate::source_inputs::SourceMap::default();
        let source = sources
            .add_checked_instance(
                "accepted/source.omg".into(),
                TEXT,
                "accepted".into(),
                Some(package),
                source_admission::SourceOrigin::User,
                source_admission::SourceResolutionStratum::Base,
                source_provenance::DependencyScope::Product,
            )
            .source_id;
        let tokens = crate::frontend_fixture::Lexer::new(TEXT)
            .tokenize()
            .unwrap();
        let syntax = crate::frontend_fixture::parse_syntax_trees_with_id(source, &tokens).unwrap();
        let resolved = crate::resolution::resolve(crate::resolution::ResolutionRequest {
            syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        })
        .unwrap();
        let typed = crate::typing::lower_symbol_resolved_trees(&resolved).unwrap();
        let machines = typed.machines();
        assert_eq!(
            machines.len(),
            1,
            "publication fixture must retain its one actual source machine"
        );
        let entry = machines.at(0);
        crate::lowering::elaborate_for(
            &typed,
            Compilation {
                declaration_metadata: &[],
                entries: &[entry.symbol],
                subjects: &[],
                purpose: source_provenance::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                calling: None,
                providers: None,
                supplies: &[],
            },
            publication,
        )
        .unwrap()
    }

    fn accepted() -> RouteVerdict {
        crate::checking::judge(Ok(elaboration(
            terminal_codec::TerminalPublication::Portable,
        )))
        .unwrap()
    }

    #[test]
    fn invocation_publication_keeps_the_judged_module_and_scope() {
        let stated = elaboration(terminal_codec::TerminalPublication::Invocation);
        let closure = accept_compilation(vec![
            crate::checking::judge(Ok(stated)).map_err(CompilationRefusal::Elaboration),
        ])
        .unwrap()
        .closure;
        let published = closure.publish_invocation().unwrap();
        assert_eq!(
            published.artifact.view().module(),
            &closure.elaboration().module
        );
        assert_eq!(published.seals.module, published.artifact.view().identity());
        assert_eq!(published.seals.seals, closure.seals());
        assert!(closure.publish().is_err());
        let portable = accept_compilation(vec![Ok(accepted())]).unwrap().closure;
        assert!(portable.publish_invocation().is_err());
    }

    #[test]
    fn later_undecided_part_cannot_publish_the_accepted_primary_closure() {
        assert!(matches!(accepted(), RouteVerdict::Accepted(_)));
        let diagnostics = accept_compilation(vec![
            Ok(accepted()),
            Ok(RouteVerdict::Incomplete(Vec::new())),
        ])
        .unwrap_err();
        assert!(diagnostics.iter().any(|entry| {
            entry
                .message
                .contains("an incomplete verdict names no goal")
        }));
        assert!(accept_compilation(Vec::new()).is_err());
    }
}

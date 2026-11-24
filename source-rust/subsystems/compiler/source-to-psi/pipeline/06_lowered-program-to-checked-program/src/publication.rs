//! Only a completely accepted compilation can issue its source for publication.
mod accepted_closure;
mod advisories;
mod mathematics;
mod optimization;
pub use accepted_closure::AcceptedClosure;
pub use advisories::AcceptedAdvisories;
pub use checked_program::CheckedPublication;

use super::verdict::{CompilationRefusal, RouteVerdict, judge_compilation};
use checked_program::CheckedProgram;
use checked_program_issuer::CHECKER;
use diagnostics::Diagnostic;
use std::sync::Arc;
use terminal_check_tables::compilation::CompilationForPublication;

/// Judge the whole lowered compilation. An accepted compilation returns the
/// source of `input` beside the checked root program, and the advisories of
/// every accepted part for whoever reports the compilation.
pub fn judge_publication<Input: CompilationForPublication>(
    input: Input,
    selections: ::optimization::PsiOptimizationSelections,
) -> Result<(CheckedPublication<Input>, AcceptedAdvisories), Vec<Diagnostic>> {
    let (program, source) = input.into_program_and_source();
    // Preserve the unsealed lowering only when there is a transformation to run.
    // Neither run of the checker receives evidence from the other.
    let candidate = (!selections.is_empty()).then(|| program.clone());
    let mut accepted = accept_compilation(judge_compilation(program))?;
    let mut original_for_diagnostics = None;
    if let Some(candidate) = candidate {
        let input = terminal_codec::terminal_psi_identity(&accepted.closure.elaboration.module)
            .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
        original_for_diagnostics = Some(accepted.closure);
        drop(accepted.additional);
        let optimized = optimization::apply(candidate, &selections)?;
        accepted = accept_compilation(judge_compilation(optimized)).map_err(|mut findings| {
            for finding in &mut findings {
                finding.message = format!(
                    "compiler defect: optimized Psi failed the lowered-form check: {}",
                    finding.message
                );
            }
            findings
        })?;
        let output = terminal_codec::terminal_psi_identity(&accepted.closure.elaboration.module)
            .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
        accepted.closure.optimization = Some(
            terminal_codec::PsiOptimizationExecutionRecord::new(selections, input, output)
                .map_err(|error| vec![Diagnostic::error(error.to_string())])?,
        );
    }
    let AcceptedCompilation {
        closure,
        additional,
    } = accepted;
    let AcceptedClosure {
        elaboration,
        discharge,
        seals,
        optimization: executed,
    } = *closure;
    let program = Arc::new(CheckedProgram::from_judgment(
        CHECKER,
        elaboration,
        seals,
        executed,
    ));
    let publication =
        CheckedPublication::from_judgment(CHECKER, source, Arc::clone(&program), additional.len());
    Ok((
        publication,
        AcceptedAdvisories::new(program, discharge, original_for_diagnostics, additional),
    ))
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
    let mut signature = None;
    let mut diagnostics = Vec::new();
    for (position, verdict) in verdicts.into_iter().enumerate() {
        match verdict {
            Ok(RouteVerdict::AcceptedPrefix(accepted)) if position == 0 => {
                first = Some(accepted);
            }
            Ok(RouteVerdict::AcceptedPrefix(_)) => diagnostics.push(route_defect(
                "a generic form or conformance cannot depend on an unfinished Build",
            )),
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
            // The signature publishes nothing executable. Its tables are
            // written into the root module once every part is accepted.
            Ok(RouteVerdict::Laws(judged)) => {
                if signature.replace(judged).is_some() {
                    diagnostics.push(route_defect("a compilation judged two signatures"));
                }
            }
            Ok(RouteVerdict::UnprovedLaws(unproved)) => {
                if unproved.is_empty() {
                    diagnostics.push(route_defect("an unproved signature names no law"));
                }
                diagnostics.extend(unproved.iter().map(|entry| entry.reject_diagnostic()));
            }
            Ok(RouteVerdict::FallBack(refused)) => {
                let mut diagnostic = Diagnostic::error(format!(
                    "not supported yet: {}; the checker does not support this construct yet, so the program is refused, with no claim that it is wrong",
                    refused.detail
                ));
                diagnostic.source_span = refused.source_span;
                diagnostics.push(diagnostic);
            }
            Err(CompilationRefusal::Elaboration(terminal_check_tables::Refusal {
                reason: terminal_check_tables::RefusalReason::Unproved(cause),
                detail,
                source_span,
                ..
            })) => {
                let message = format!("{detail} ({})", cause.describe());
                let mut diagnostic = Diagnostic::error(if cause.is_incomplete() {
                    format!("not supported yet: {message}")
                } else {
                    message
                });
                diagnostic.source_span = source_span;
                diagnostics.push(diagnostic);
            }
            Err(CompilationRefusal::Elaboration(refused)) if refused.source_span.is_some() => {
                let mut diagnostic = route_defect(&refused.detail);
                diagnostic.source_span = refused.source_span;
                diagnostics.push(diagnostic);
            }
            Err(defect) => diagnostics.push(route_defect(&format!("{defect:?}"))),
        }
    }
    if !diagnostics.is_empty() {
        // A template and its applications can report the same error at the
        // same authored site. Keep the first occurrence in traversal order.
        let mut reported = std::collections::HashSet::new();
        diagnostics.retain(|diagnostic| {
            diagnostic
                .source_span
                .is_none_or(|span| reported.insert((span, diagnostic.message.clone())))
        });
        return Err(diagnostics);
    }
    let mut closure = first.ok_or_else(|| vec![route_defect("no accepted compilation root")])?;
    if let Some(signature) = signature {
        mathematics::publish(&mut closure.elaboration.module, &signature)
            .map_err(|detail| vec![route_defect(detail)])?;
    }
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

    #[test]
    fn an_unsupported_contract_goal_identifies_its_proof_limit() {
        use semantic_vocabulary::{ObligationId, Proposition, PropositionContext, PropositionId};
        use terminal_check_tables::{RefusalReason, RefusalSite, UnprovedCause};

        let identity = PropositionId::new(1).unwrap();
        let refusal = crate::checking::derive_contract_obligation(
            RefusalSite::Program,
            &PropositionContext::default(),
            ObligationId::new(1).unwrap(),
            &Proposition::Atom(identity),
            &[],
            &mut Default::default(),
        )
        .expect_err("an opaque proposition without premises has no proof");
        assert_eq!(
            refusal.reason,
            RefusalReason::Unproved(UnprovedCause::NoRuleForGoalForm)
        );
        let diagnostics =
            accept_compilation(vec![Err(CompilationRefusal::Elaboration(refusal))]).unwrap_err();
        let [diagnostic] = diagnostics.as_slice() else {
            panic!("expected one proof limit")
        };
        assert!(diagnostic.message.starts_with("not supported yet:"));
        assert!(
            diagnostic
                .message
                .contains(UnprovedCause::NoRuleForGoalForm.describe())
        );
        assert!(!diagnostic.message.contains("defect"));
    }

    #[test]
    fn checker_defect_retains_its_authored_location() {
        use terminal_check_tables::{Refusal, RefusalReason, RefusalSite};

        let span =
            source_text::SourceSpan::new(source_text::SourceId(3), source_text::Span::new(12, 19));
        let refusal = Refusal::at(
            RefusalReason::ForeignIdentity,
            RefusalSite::Program,
            "a structural result lost its declared domain",
        )
        .with_source_span(Some(span));

        let diagnostics =
            accept_compilation(vec![Err(CompilationRefusal::Elaboration(refusal))]).unwrap_err();
        let [diagnostic] = diagnostics.as_slice() else {
            panic!("expected one checker defect")
        };
        assert_eq!(diagnostic.source_span, Some(span));
        assert!(
            diagnostic
                .message
                .contains("a structural result lost its declared domain")
        );
        assert!(diagnostic.message.contains("defect of the checker"));
    }

    fn elaboration(
        publication: terminal_codec::TerminalPublication,
    ) -> crate::lowering::Elaboration {
        elaboration_with(publication, "machine entry() crashes none {}", None)
    }

    fn elaboration_with(
        publication: terminal_codec::TerminalPublication,
        text: &str,
        pending_call: Option<&str>,
    ) -> crate::lowering::Elaboration {
        elaboration_retaining(publication, text, pending_call, &[])
    }

    fn elaboration_retaining(
        publication: terminal_codec::TerminalPublication,
        text: &str,
        pending_call: Option<&str>,
        saved: &[&str],
    ) -> crate::lowering::Elaboration {
        let package = semantic_vocabulary::PackageKeyIdentity::from_digest([0xa7; 32]).unwrap();
        let mut sources = crate::source_inputs::SourceMap::default();
        let source = sources
            .add_checked_instance(
                "accepted/source.omg".into(),
                text,
                "accepted".into(),
                Some(package),
                source_admission::SourceOrigin::User,
                source_admission::SourceResolutionStratum::Base,
                source_provenance::DependencyScope::Product,
            )
            .source_id;
        let tokens = crate::frontend_fixture::Lexer::new(text)
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
        let mut typed = crate::typing::lower_symbol_resolved_trees(&resolved).unwrap();
        let entry = typed.machines().at(0).symbol;
        if let Some(name) = pending_call {
            let start = text.find(name).unwrap();
            typed
                .parked_build_locals
                .push(typed_trees::ParkedBuildLocals {
                    entry: typed.symbols.symbol_source_span(entry).unwrap(),
                    state: None,
                    pending_completion_at: source_text::SourceSpan::new(
                        source,
                        source_text::Span::new(start, start + name.len()),
                    ),
                    declarations: saved
                        .iter()
                        .map(|name| {
                            let start = text.find(name).unwrap();
                            source_text::Span::new(start, start + name.len())
                        })
                        .collect(),
                    deferred_subjects: Vec::new(),
                });
        }
        crate::lowering::elaborate_for(
            &typed,
            Compilation {
                declaration_metadata: &[],
                entries: &[entry],
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
    fn a_projected_unit_entry_cannot_be_published_as_a_complete_program() {
        // Passing an owned temporary gives the complete body a real
        // post-call home-retirement edge. The parked body must leave before it.
        let source = "data Ticket MARK { code: i32; } machine entry() suspends { suspend pause(Ticket { code: 3 }); } machine pause(ticket: Ticket) suspends {}";
        for marker in ["", "[copy]"] {
            let source = source.replace("MARK", marker);
            let publication = terminal_codec::TerminalPublication::Invocation;
            let complete = elaboration_with(publication, &source, None);
            let stated = elaboration_with(publication, &source, Some("pause"));
            let RouteVerdict::AcceptedPrefix(closure) = crate::checking::judge(Ok(stated)).unwrap()
            else {
                panic!("a projected unit entry still owes its suffix");
            };
            let invocation = closure.publish_invocation().unwrap();
            let view = invocation.artifact.view();
            let [(owner, call)] = view.pending_normal_completions() else {
                panic!("the open entry must own exactly one mandatory wait");
            };
            let full = complete.module.machine(*owner).unwrap();
            assert!(full.blocks.iter().any(|block| {
                block.operations.last().is_some_and(|op| op.id == *call)
                    && matches!(block.terminator, terminal_psi::Terminator::Jump { .. })
            }));
            let RouteVerdict::Accepted(closure_full) =
                crate::checking::judge(Ok(complete)).unwrap()
            else {
                panic!("the ordinary call returns normally");
            };
            let published_full = closure_full.publish_invocation().unwrap();
            terminal_semantics::continuation::CheckedPrefix::check(
                view.module(),
                (*owner, *call),
                published_full.artifact.view().module(),
                (*owner, *call),
            )
            .expect("the outgoing call parks everything its real normal successor retires");
            assert_eq!(*owner, view.module().machines[0].id);
            assert!(view.closed_module().is_none());
            assert!(closure.publish().is_err());
        }
    }

    #[test]
    fn parking_an_owner_preserves_the_exclusive_borrow_of_its_field() {
        let source = r"
machine entry() suspends {
    let mutable gauge: Gauge = Gauge { amount: 3, neighbor: 19 };
    let held: @mutable i32 = @mutable gauge.amount;
    OBSERVE
    suspend pause();
}
data Gauge { amount: i32; neighbor: i32; }
machine pause() suspends {}
";
        for (observe, legal) in [("", true), ("let conflict: i32 = gauge.amount;", false)] {
            let stated = elaboration_retaining(
                terminal_codec::TerminalPublication::Invocation,
                &source.replace("OBSERVE", observe),
                Some("pause"),
                &["gauge", "held"],
            );
            let verdict = crate::checking::judge(Ok(stated)).unwrap();
            if legal {
                assert!(
                    matches!(verdict, RouteVerdict::AcceptedPrefix(_)),
                    "{verdict:?}"
                );
            } else {
                let diagnostics = accept_compilation(vec![Ok(verdict)]).unwrap_err();
                assert!(
                    diagnostics
                        .iter()
                        .any(|d| d.message.contains("exclusive reference")),
                    "{diagnostics:?}"
                );
            }
        }
    }

    #[test]
    fn a_value_body_can_publish_only_its_checked_invocation_prefix() {
        let publication = terminal_codec::TerminalPublication::Invocation;
        let mut stated = elaboration_with(
            publication,
            "machine entry() -> i32 suspends { suspend pause(); } machine pause() suspends {}",
            Some("pause"),
        );
        let machine = &stated.module.machines[0];
        let declaration = machine.declaration.as_ref().unwrap().clone();
        let callable = declaration.callable;
        let scope = &mut stated.module.scalars.scalar_applications.proof_scope;
        let id =
            terminal_psi::ProofDeclarationId(u32::try_from(scope.declarations.len() + 1).unwrap());
        scope
            .declarations
            .insert(terminal_psi::ProofScopeDeclaration::Callable(declaration));
        scope
            .callables
            .insert(terminal_psi::ProofCallableAssociation::Callable {
                declaration: id,
                callable: terminal_psi::TargetMachineCallable::Machine(machine.id),
            });
        let header = scope
            .callable_headers
            .insert(terminal_psi::ProofCallableHeader {
                declaration: id,
                callable: Ok(callable),
                ..Default::default()
            });
        let mut forged = stated.clone();
        let open = forged
            .module
            .machines
            .iter_mut()
            .find(|machine| {
                machine.blocks.iter().any(|block| {
                    matches!(
                        block.terminator,
                        terminal_psi::Terminator::PendingNormalCompletion { .. }
                    )
                })
            })
            .unwrap();
        let unreturned = open.result.scalar().unwrap().id;
        let scalars = open
            .blocks
            .iter_mut()
            .find_map(|block| match &mut block.terminator {
                terminal_psi::Terminator::PendingNormalCompletion { scalars, .. } => Some(scalars),
                _ => None,
            })
            .unwrap();
        scalars.push(unreturned);
        let diagnostics = accept_compilation(vec![
            crate::checking::judge(Ok(forged)).map_err(CompilationRefusal::Elaboration),
        ])
        .unwrap_err();
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("a live scalar has no declaration in its machine")),
            "{diagnostics:?}"
        );
        let RouteVerdict::AcceptedPrefix(closure) = crate::checking::judge(Ok(stated)).unwrap()
        else {
            panic!("the partial body must receive its own checked verdict");
        };
        let invocation = closure.publish_invocation().unwrap();
        let view = invocation.artifact.view();
        assert_eq!(view.pending_normal_completions().len(), 1);
        assert!(
            !view
                .module()
                .scalars
                .scalar_applications
                .proof_scope
                .callable_headers
                .get(header)
                .supplies_checked_body
        );
        let (owner, call) = view.pending_normal_completions()[0];
        let machine = view
            .module()
            .machines
            .iter()
            .find(|row| row.id == owner)
            .unwrap();
        assert!(matches!(
            machine.result,
            terminal_psi::TerminalMachineResult::Scalar(_)
        ));
        assert!(machine.blocks.iter().any(|block| matches!(
            block.terminator,
            terminal_psi::Terminator::PendingNormalCompletion { call: cut, .. } if cut == call
        )));
        assert!(view.closed_module().is_none());
        assert!(closure.publish().is_err());
        assert!(terminal_codec::ClosedTerminalModule::new(view.module().clone()).is_err());
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

//! Only a completely accepted compilation can issue its source for publication.
mod accepted_closure;
mod advisories;
mod mathematics;
mod optimization;
pub use accepted_closure::AcceptedClosure;
pub use advisories::AcceptedAdvisories;
pub use checked_program::CheckedPublication;

use super::verdict::{CompilationRefusal, RouteVerdict, judge_compilation_with_policy};
use checked_program::CheckedProgram;
use checked_program_issuer::CHECKER;
use diagnostics::Diagnostic;
use std::sync::Arc;
use terminal_check_tables::UnprovedCause;
use terminal_check_tables::compilation::CompilationForPublication;

/// Judge the whole lowered compilation under the default proof-work policy.
pub fn judge_publication<Input: CompilationForPublication>(
    input: Input,
    selections: ::optimization::PsiOptimizationSelections,
) -> Result<(CheckedPublication<Input>, AcceptedAdvisories), Vec<Diagnostic>> {
    judge_publication_with_policy(input, selections, crate::ProofWorkPolicy::default())
}

/// Judge the whole lowered compilation under the root Build's proof-work policy.
/// An accepted compilation returns the source of `input` beside the checked root
/// program, and the advisories of every accepted part for whoever reports the
/// compilation.
pub fn judge_publication_with_policy<Input: CompilationForPublication>(
    input: Input,
    selections: ::optimization::PsiOptimizationSelections,
    policy: crate::ProofWorkPolicy,
) -> Result<(CheckedPublication<Input>, AcceptedAdvisories), Vec<Diagnostic>> {
    let (program, source) = input.into_program_and_source();
    // Preserve the unsealed lowering only when there is a transformation to run.
    // Neither run of the checker receives evidence from the other.
    let candidate = (!selections.is_empty()).then(|| program.clone());
    let mut accepted = accept_compilation(judge_compilation_with_policy(program, policy))?;
    let mut original_for_diagnostics = None;
    if let Some(candidate) = candidate {
        let input = terminal_codec::terminal_psi_identity(&accepted.closure.elaboration.module)
            .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
        original_for_diagnostics = Some(accepted.closure);
        drop(accepted.additional);
        let optimized = optimization::apply(candidate, &selections)?;
        accepted = accept_compilation(judge_compilation_with_policy(optimized, policy)).map_err(
            |mut findings| {
                for finding in &mut findings {
                    finding.message = format!(
                        "compiler defect: optimized Psi failed the lowered-form check: {}",
                        finding.message
                    );
                }
                findings
            },
        )?;
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
    let additional: Vec<_> = additional
        .into_iter()
        .map(|part| (Arc::new(part.elaboration), part.discharge))
        .collect();
    let forms = additional
        .iter()
        .map(|(form, _)| Arc::clone(form))
        .collect();
    let publication =
        CheckedPublication::from_judgment(CHECKER, source, Arc::clone(&program), forms);
    Ok((
        publication,
        AcceptedAdvisories::new(program, discharge, original_for_diagnostics, additional),
    ))
}

/// The root, generic forms and witnesses of a completely accepted compilation.
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
                    let note = match entry.cause {
                        UnprovedCause::BudgetExhausted | UnprovedCause::GoalStepsExhausted { .. } => {
                            "proof search ran out of steps, so the checker could not judge it either way"
                        }
                        _ => "the checker could not judge it either way",
                    };
                    diagnostic.message =
                        format!("not supported yet: {}; {note}", diagnostic.message);
                    goal_form_provision(diagnostic, entry.cause, Some(entry.obligation))
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
            Err(CompilationRefusal::Elaboration(
                refused @ terminal_check_tables::Refusal {
                    reason: terminal_check_tables::RefusalReason::Unproved(cause),
                    ..
                },
            )) => {
                let message = format!("{} ({})", refused.detail, cause.describe());
                let mut diagnostic = Diagnostic::error(if cause.is_incomplete() {
                    format!("not supported yet: {message}")
                } else {
                    message
                });
                diagnostic.source_span = refused.source_span;
                diagnostics.push(goal_form_provision(
                    diagnostic,
                    cause,
                    refused.unproved_obligation(),
                ));
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

/// A goal whose form the prover has no rule for, or whose search ran out of
/// its step budget, leaves the request `Incomplete` at its resource code,
/// naming the obligation: code 36, `coverage_proof_goal_forms`, or code 37,
/// `proof_search_goal_steps`, with the limit that goal was given
/// (`wiki/spec/build/compiler_request.md`, "Unproved goal form
/// publication"). Every other cause keeps its diagnostic as it is. Without
/// an obligation identity the provision cannot be named, so the diagnostic
/// stays a refusal rather than an `Incomplete` with an invented coordinate.
fn goal_form_provision(
    diagnostic: Diagnostic,
    cause: UnprovedCause,
    obligation: Option<semantic_vocabulary::ObligationId>,
) -> Diagnostic {
    use diagnostics::CoverageForm;
    let Some(obligation) = obligation else {
        return diagnostic;
    };
    let (form, provision) = match cause {
        UnprovedCause::NoRuleForGoalForm => (
            CoverageForm::ProofGoalForm,
            CoverageForm::ProofGoalForm.exceeded(obligation.get()),
        ),
        UnprovedCause::GoalStepsExhausted { limit } => (
            CoverageForm::ProofGoalSteps,
            CoverageForm::ProofGoalSteps.exceeded_at(obligation.get(), u64::from(limit)),
        ),
        _ => return diagnostic,
    };
    Diagnostic {
        message: format!(
            "{} [Incomplete code {} ({}): obligation {}]",
            diagnostic.message,
            form.code(),
            form.resource(),
            obligation.get()
        ),
        exceeded_provision: Some(provision),
        ..diagnostic
    }
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
        let refusal = crate::derive_contract_obligation(
            RefusalSite::Program,
            &PropositionContext::default(),
            ObligationId::new(7).unwrap(),
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
        // Code 36 names the obligation the prover has no rule for.
        assert_eq!(
            diagnostic.exceeded_provision,
            Some(diagnostics::CoverageForm::ProofGoalForm.exceeded(7))
        );
        assert!(
            diagnostic
                .message
                .contains("[Incomplete code 36 (coverage_proof_goal_forms): obligation 7]"),
            "{}",
            diagnostic.message
        );
    }

    /// A goal of a form the prover has rules for, left unproved, is a
    /// refusal: it names no coverage provision, so the request rejects.
    #[test]
    fn an_unproved_goal_with_rules_names_no_coverage_provision() {
        use semantic_vocabulary::{ObligationId, Proposition, PropositionContext};
        use terminal_check_tables::{RefusalReason, RefusalSite, UnprovedCause};

        let refusal = crate::derive_contract_obligation(
            RefusalSite::Program,
            &PropositionContext::default(),
            ObligationId::new(7).unwrap(),
            &Proposition::Falsehood,
            &[],
            &mut Default::default(),
        )
        .expect_err("nothing proves falsehood without premises");
        assert_eq!(
            refusal.reason,
            RefusalReason::Unproved(UnprovedCause::NoDerivation)
        );
        assert_eq!(refusal.unproved_obligation(), ObligationId::new(7));
        let diagnostics =
            accept_compilation(vec![Err(CompilationRefusal::Elaboration(refusal))]).unwrap_err();
        let [diagnostic] = diagnostics.as_slice() else {
            panic!("expected one refusal")
        };
        assert_eq!(diagnostic.exceeded_provision, None);
        assert!(!diagnostic.message.contains("coverage_proof_goal_forms"));
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
        elaboration_with(publication, "machine entry() crashes none {}")
    }

    fn elaboration_with(
        publication: terminal_codec::TerminalPublication,
        text: &str,
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
        let typed = crate::typing::lower_symbol_resolved_trees(&resolved).unwrap();
        let entry = typed.machines().at(0).symbol;
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
        crate::judge(Ok(elaboration(
            terminal_codec::TerminalPublication::Portable,
        )))
        .unwrap()
    }

    #[test]
    fn invocation_publication_keeps_the_judged_module_and_scope() {
        let stated = elaboration(terminal_codec::TerminalPublication::Invocation);
        let closure = accept_compilation(vec![
            crate::judge(Ok(stated)).map_err(CompilationRefusal::Elaboration),
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
    fn a_suspending_call_keeps_the_normal_successor_for_owned_and_copied_arguments() {
        let source = "data Ticket MARK { code: i32; } machine entry() suspends { suspend pause(Ticket { code: 3 }); } machine pause(ticket: Ticket) suspends {}";
        for marker in ["", "[copy]"] {
            let source = source.replace("MARK", marker);
            let stated = elaboration_with(terminal_codec::TerminalPublication::Invocation, &source);
            let RouteVerdict::Accepted(closure) = crate::judge(Ok(stated)).unwrap() else {
                panic!("the call and its normal successor must check together");
            };
            let invocation = closure.publish_invocation().unwrap();
            let view = invocation.artifact.view();
            let entry = &view.module().machines[0];
            let (block, call) = entry
                .blocks
                .iter()
                .find_map(|block| {
                    let call = block.operations.last()?;
                    call.suspension_crossing.map(|_| (block, call))
                })
                .expect("the complete entry has one suspending call");
            let [argument] = call.kind.structural_arguments() else {
                panic!("the call passes the temporary Ticket");
            };
            let terminal_psi::Terminator::Jump {
                target,
                cleanup_actions,
                ..
            } = &block.terminator
            else {
                panic!("the call must reach its normal successor");
            };
            if marker == "[copy]" {
                assert!(
                    cleanup_actions.contains(
                        &terminal_psi::TerminalAffineCleanupAction::RetireHome(argument.place)
                    ),
                    "{cleanup_actions:?}"
                );
            }
            assert!(entry.blocks.iter().any(|next| next.id == *target
                && matches!(next.terminator, terminal_psi::Terminator::ReturnUnit { .. })));
        }
    }

    #[test]
    fn a_field_borrow_used_after_suspension_excludes_conflicting_reads() {
        let source = r"
machine entry() suspends {
    let mutable gauge: Gauge = Gauge { amount: 3, neighbor: 19 };
    let held: @mutable i32 = @mutable gauge.amount;
    OBSERVE
    suspend pause();
    $held = 7;
}
data Gauge { amount: i32; neighbor: i32; }
machine pause() suspends {}
";
        for (observe, legal) in [("", true), ("let conflict: i32 = gauge.amount;", false)] {
            let stated = elaboration_with(
                terminal_codec::TerminalPublication::Invocation,
                &source.replace("OBSERVE", observe),
            );
            let verdict = crate::judge(Ok(stated)).unwrap();
            if legal {
                assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
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

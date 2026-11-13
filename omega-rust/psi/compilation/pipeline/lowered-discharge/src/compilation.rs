//! Build execution and product compilation use the same source verdict and
//! publish the same checked closure. Every generic part must pass before code
//! is published, and a publication failure retains its actual reason.

use diagnostics::Diagnostic;
use structure_elaboration::Compilation;
use typed_trees::FrozenTypedTrees;

use crate::{CompilationRefusal, RoutePublication, RouteVerdict};

/// Judge every part of this compilation, then publish only its executable
/// closure. The source judgment stays separate from publication so an accepted
/// source with an unsupported artifact cannot silently select another producer.
pub fn judge_and_publish(
    typed: &FrozenTypedTrees,
    compilation: Compilation<'_>,
) -> (
    CompilationJudgment,
    Result<RoutePublication, Vec<Diagnostic>>,
) {
    let verdicts = crate::judge_compilation(typed, compilation);
    let mut program = None;
    let judgments = verdicts
        .into_iter()
        .enumerate()
        .map(|(position, verdict)| match (position, verdict) {
            (0, Ok(RouteVerdict::Accepted(closure))) => {
                program = Some(closure);
                CompilationJudgment::Accepted
            }
            (_, verdict) => CompilationJudgment::of(verdict),
        })
        .collect();
    let judgment = CompilationJudgment::all(judgments);
    let published = match &judgment {
        CompilationJudgment::Accepted => program
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "accepted source checking retained no executable program",
                )]
            })
            .and_then(|closure| closure.publish().map_err(publication_diagnostics)),
        CompilationJudgment::Rejected(diagnostics) | CompilationJudgment::Refused(diagnostics) => {
            Err(diagnostics.clone())
        }
    };
    (judgment, published)
}

fn publication_diagnostics(error: crate::PublicationError) -> Vec<Diagnostic> {
    use crate::PublicationError;
    let reason = match error {
        PublicationError::GenericCheckForm => {
            "the accepted module still contains abstract generic requirements".to_owned()
        }
        PublicationError::Undeclared(machine, reason) => {
            format!(
                "machine {} has no portable declaration: {reason}",
                machine.get()
            )
        }
        PublicationError::Semantic(reason) => format!("checked program publication: {reason}"),
        PublicationError::Artifact(error) => format!("checked program artifact: {error}"),
        PublicationError::Seals(error) => format!("checked program evidence: {error:?}"),
    };
    vec![Diagnostic::error(format!("{NO_PRODUCT_ROUTE}: {reason}"))]
}

/// Diagnostic prefix when an accepted module cannot be published as a product.
pub const NO_PRODUCT_ROUTE: &str = "no product route yet";

/// The first sentence of the error a compilation reports when the route
/// cannot judge the program.
pub const NOT_SUPPORTED_YET: &str = "not supported yet";

/// What the route decided.
#[derive(Debug)]
pub enum CompilationJudgment {
    /// Every obligation of the closure was discharged.
    Accepted,
    /// The program is rejected, with these diagnostics. Never empty.
    Rejected(Vec<Diagnostic>),
    /// The route cannot judge the program: an unserved construct, a goal form
    /// the prover has no rule for, or a defect of the route. One diagnostic
    /// per cause, each opening with `NOT_SUPPORTED_YET`. Never empty.
    Refused(Vec<Diagnostic>),
}

impl CompilationJudgment {
    /// The judgment of a compilation made of several separately judged
    /// parts: refused when any part is, with every refused part's
    /// diagnostics; otherwise rejected with every part's diagnostics when any
    /// part is rejected; accepted when all are. A refusal comes first because
    /// the route has not judged the whole compilation, so it reports what it
    /// cannot judge rather than a partial list of failures.
    pub fn all(parts: Vec<Self>) -> Self {
        let mut refused: Option<Vec<Diagnostic>> = None;
        let mut rejected: Option<Vec<Diagnostic>> = None;
        for part in parts {
            match part {
                Self::Refused(diagnostics) => {
                    refused.get_or_insert_with(Vec::new).extend(diagnostics);
                }
                Self::Rejected(diagnostics) => {
                    rejected.get_or_insert_with(Vec::new).extend(diagnostics);
                }
                Self::Accepted => {}
            }
        }
        match (refused, rejected) {
            (Some(refused), _) => Self::Refused(refused),
            (None, Some(rejected)) => Self::Rejected(rejected),
            (None, None) => Self::Accepted,
        }
    }

    pub fn of(verdict: Result<RouteVerdict, CompilationRefusal>) -> Self {
        let verdict = match verdict {
            Ok(verdict) => verdict,
            Err(defect) => return Self::Refused(vec![route_defect(&format!("{defect:?}"))]),
        };
        match verdict {
            RouteVerdict::Accepted(_) => Self::Accepted,
            RouteVerdict::FallBack(refusal) => Self::Refused(vec![Diagnostic::error(format!(
                "{NOT_SUPPORTED_YET}: {}; the checker does not support this construct yet, so \
                 the program is refused, with no claim that it is wrong",
                refusal.detail
            ))]),
            RouteVerdict::Incomplete(unproved) if !unproved.is_empty() => Self::Refused(
                unproved
                    .iter()
                    .map(|entry| {
                        let mut diagnostic = entry.reject_diagnostic();
                        diagnostic.message = format!(
                            "{NOT_SUPPORTED_YET}: {}; the checker cannot prove a goal of this \
                             form yet, so the program is refused, with no claim that it is wrong",
                            diagnostic.message
                        );
                        diagnostic
                    })
                    .collect(),
            ),
            RouteVerdict::Incomplete(_) => {
                Self::Refused(vec![route_defect("an incomplete verdict names no goal")])
            }
            RouteVerdict::Rejected {
                formations,
                unproved,
                unestablished,
                conflicts,
                exceeded,
            } => {
                let mut reported = Vec::new();
                reported.extend(formations.iter().map(|entry| entry.reject_diagnostic()));
                // A rejection may list goals of a form the prover has no
                // rule for beside the ones it failed; those are no claim
                // about the program and are left out.
                for entry in unproved.iter().filter(|entry| !entry.cause.is_incomplete()) {
                    reported.push(entry.reject_diagnostic());
                    reported.extend(entry.opening_diagnostic());
                }
                reported.extend(unestablished.iter().map(|entry| entry.reject_diagnostic()));
                reported.extend(conflicts.iter().map(|entry| entry.reject_diagnostic()));
                reported.extend(exceeded.iter().map(|entry| entry.reject_diagnostic()));
                if reported.is_empty() {
                    // `Rejected` is never empty; one that is would be a
                    // defect of the route, which must not decide the program
                    // either way.
                    return Self::Refused(vec![route_defect("a rejection names no failure")]);
                }
                Self::Rejected(reported)
            }
        }
    }

    /// The verdict as the check reports it: the diagnostics the compilation
    /// fails with, unless the route accepted.
    pub fn verdict(self) -> Result<(), Vec<Diagnostic>> {
        match self {
            Self::Accepted => Ok(()),
            Self::Rejected(diagnostics) | Self::Refused(diagnostics) => Err(diagnostics),
        }
    }
}

/// The refusal for a route that returned no usable verdict.
fn route_defect(detail: &str) -> Diagnostic {
    Diagnostic::error(format!(
        "{NOT_SUPPORTED_YET}: the checker could not judge this program ({detail}); this is a \
         defect of the checker, and the program is refused rather than accepted without a verdict"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rejected(message: &str) -> CompilationJudgment {
        CompilationJudgment::Rejected(vec![Diagnostic::error(message)])
    }

    fn refused(message: &str) -> CompilationJudgment {
        CompilationJudgment::Refused(vec![Diagnostic::error(message)])
    }

    fn messages(outcome: Result<(), Vec<Diagnostic>>) -> Vec<String> {
        outcome
            .expect_err("the compilation fails")
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect()
    }

    /// A compilation judged in parts is refused when any part is, rejected
    /// with every rejected part's diagnostics, and accepted otherwise.
    #[test]
    fn parts_combine() {
        assert!(matches!(
            CompilationJudgment::all(vec![
                CompilationJudgment::Accepted,
                CompilationJudgment::Accepted
            ]),
            CompilationJudgment::Accepted
        ));
        assert_eq!(
            messages(
                CompilationJudgment::all(vec![rejected("a"), refused("x"), refused("y")]).verdict()
            ),
            ["x", "y"]
        );
        assert_eq!(
            messages(
                CompilationJudgment::all(vec![
                    rejected("a"),
                    CompilationJudgment::Accepted,
                    rejected("b")
                ])
                .verdict()
            ),
            ["a", "b"]
        );
    }

    /// A verdict that names nothing is a defect of the route, and a defect
    /// refuses the program rather than deciding it either way.
    #[test]
    fn a_verdict_that_names_nothing_refuses_the_program() {
        for verdict in [
            RouteVerdict::Incomplete(Vec::new()),
            RouteVerdict::Rejected {
                formations: Vec::new(),
                unproved: Vec::new(),
                unestablished: Vec::new(),
                conflicts: Vec::new(),
                exceeded: Vec::new(),
            },
        ] {
            let reported = messages(CompilationJudgment::of(Ok(verdict)).verdict());
            assert!(reported[0].starts_with(NOT_SUPPORTED_YET), "{reported:#?}");
        }
    }
}

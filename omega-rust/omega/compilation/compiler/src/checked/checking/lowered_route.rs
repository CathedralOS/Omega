//! Source checking decides whether a program is valid and publishes its checked
//! portable module with evidence seals. Every product consumes that result;
//! refusal or failed publication never retries a retired checker or producer.
//!
//! The executable closure and each generic body's admitted alternatives must
//! all pass before the closure is published. Explicit entry changes use this
//! same path and receive new evidence for the selected module.

use diagnostics::Diagnostic;
use lowered_discharge::{CompilationRefusal, RouteVerdict};
use semantic_vocabulary::PackageKeyIdentity;
use symbols::SymbolHandle;

/// What the route judges: the declarations the request claims, which are
/// the same `structure_elaboration::elaborate` roots.
pub(super) struct RouteScope<'a> {
    pub(super) entries: &'a [SymbolHandle],
    pub(super) subject: Option<PackageKeyIdentity>,
    pub(super) selected: &'a [SymbolHandle],
    pub(super) bindings: &'a [typed_trees::callable_signature_shapes::BoundCallableShape],
}

impl RouteScope<'_> {
    /// The route's judgment of `typed`: the closure of the compilation's
    /// roots, and the check form of each generic machine the subject
    /// declares. generics.md, "Static type equality": "An unspecialized body
    /// checks every admitted alternative; one consumer cannot authorize an
    /// otherwise invalid generic body." So a generic body is judged as
    /// written whether or not the program applies it, and the machines stage
    /// 04 generated for the program's own applications are judged as closed
    /// machines of the closure. The route accepts only when it accepts every
    /// one of them.
    ///
    /// The source verdict and executable publication are separate results.
    /// A valid program whose checked module cannot be published keeps that
    /// exact failure; product emission must never substitute another module.
    /// Only the first verdict owns executable code. The remaining verdicts
    /// check generic bodies and must all pass before publication is attempted.
    pub(super) fn judge(
        &self,
        typed: &std::sync::Arc<typed_trees::FrozenTypedTrees>,
    ) -> (
        RouteJudgment,
        Result<std::sync::Arc<compilation_products::CheckedApplicationProgram>, Vec<Diagnostic>>,
    ) {
        let verdicts = lowered_discharge::judge_compilation(
            typed,
            structure_elaboration::Compilation {
                entries: self.entries,
                subject: self.subject,
                selected: self.selected,
                bindings: self.bindings,
            },
        );
        let mut program = None;
        let judgments = verdicts
            .into_iter()
            .enumerate()
            .map(|(position, verdict)| match (position, verdict) {
                (0, Ok(RouteVerdict::Accepted(closure))) => {
                    program = Some(closure);
                    RouteJudgment::Accepted
                }
                (_, verdict) => RouteJudgment::of(verdict),
            })
            .collect();
        let judgment = RouteJudgment::all(judgments);
        let published = match &judgment {
            RouteJudgment::Accepted => program
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "accepted source checking retained no executable program",
                    )]
                })
                .and_then(|closure| closure.publish().map_err(publication_diagnostics))
                .and_then(|publication| {
                    compilation_products::CheckedApplicationProgram::new(
                        compilation_products::ProductSourceDeclarations {
                            typed: std::sync::Arc::clone(typed),
                            machines: publication.machines,
                            boundaries: publication.boundaries,
                            records: publication.source_records.into(),
                            services: publication.services.into(),
                        },
                        std::sync::Arc::new(publication.artifact),
                        publication.seals,
                    )
                    .map(std::sync::Arc::new)
                }),
            RouteJudgment::Rejected(diagnostics) | RouteJudgment::Refused(diagnostics) => {
                Err(diagnostics.clone())
            }
        };
        (judgment, published)
    }
}

/// Check explicitly selected roots without rewriting an accepted artifact's
/// entry or evidence seals. Keep the original source obligations alongside the
/// new entries, and check every newly installed entry binding.
pub(crate) fn publish_selected_roots(
    source: &std::sync::Arc<compilation_products::CheckedApplicationProgram>,
    entries: &[SymbolHandle],
    bindings: &[typed_trees::callable_signature_shapes::BoundCallableShape],
) -> Result<std::sync::Arc<compilation_products::CheckedApplicationProgram>, Vec<Diagnostic>> {
    let declarations = source.correspondence();
    let module = source.module();
    let selected = module
        .machines
        .iter()
        .map(|machine| {
            declarations
                .machines
                .get(machine.id)
                .copied()
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "root selection lost a checked machine's source declaration",
                    )]
                })
        })
        .chain(module.boundary_machines.iter().map(|boundary| {
            declarations
                .boundaries
                .get(boundary.id)
                .copied()
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "root selection lost a checked boundary's source declaration",
                    )]
                })
        }))
        .collect::<Result<Vec<_>, _>>()?;
    let (verdict, published) = RouteScope {
        entries,
        subject: None,
        selected: &selected,
        bindings,
    }
    .judge(source.typed());
    verdict.verdict()?;
    published
}

fn publication_diagnostics(error: lowered_discharge::PublicationError) -> Vec<Diagnostic> {
    use lowered_discharge::PublicationError;
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
pub(crate) const NO_PRODUCT_ROUTE: &str = "no product route yet";

/// The first sentence of the error a compilation reports when the route
/// cannot judge the program.
pub(crate) const NOT_SUPPORTED_YET: &str = "not supported yet";

/// What the route decided.
#[derive(Debug)]
pub(super) enum RouteJudgment {
    /// Every obligation of the closure was discharged.
    Accepted,
    /// The program is rejected, with these diagnostics. Never empty.
    Rejected(Vec<Diagnostic>),
    /// The route cannot judge the program: an unserved construct, a goal form
    /// the prover has no rule for, or a defect of the route. One diagnostic
    /// per cause, each opening with `NOT_SUPPORTED_YET`. Never empty.
    Refused(Vec<Diagnostic>),
}

impl RouteJudgment {
    /// The judgment of a compilation made of several separately judged
    /// parts: refused when any part is, with every refused part's
    /// diagnostics; otherwise rejected with every part's diagnostics when any
    /// part is rejected; accepted when all are. A refusal comes first because
    /// the route has not judged the whole compilation, so it reports what it
    /// cannot judge rather than a partial list of failures.
    pub(super) fn all(parts: Vec<Self>) -> Self {
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

    pub(super) fn of(verdict: Result<RouteVerdict, CompilationRefusal>) -> Self {
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
    pub(super) fn verdict(self) -> Result<(), Vec<Diagnostic>> {
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

    fn rejected(message: &str) -> RouteJudgment {
        RouteJudgment::Rejected(vec![Diagnostic::error(message)])
    }

    fn refused(message: &str) -> RouteJudgment {
        RouteJudgment::Refused(vec![Diagnostic::error(message)])
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
            RouteJudgment::all(vec![RouteJudgment::Accepted, RouteJudgment::Accepted]),
            RouteJudgment::Accepted
        ));
        assert_eq!(
            messages(RouteJudgment::all(vec![rejected("a"), refused("x"), refused("y")]).verdict()),
            ["x", "y"]
        );
        assert_eq!(
            messages(
                RouteJudgment::all(vec![rejected("a"), RouteJudgment::Accepted, rejected("b")])
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
            let reported = messages(RouteJudgment::of(Ok(verdict)).verdict());
            assert!(reported[0].starts_with(NOT_SUPPORTED_YET), "{reported:#?}");
        }
    }
}

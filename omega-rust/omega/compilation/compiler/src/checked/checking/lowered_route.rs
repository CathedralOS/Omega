// The lowered-form checker's verdict is the product compilation's verdict.
//
// The owner ruled on 2026-11-12 that the old checker (Psi 05, validation and
// Psi 06) is removed now, not once every program is covered
// (wiki/drafts/designs/check_on_lowered_form.md, "The gate"): none of the
// obligations it accepts is backed by evidence the proof kernel re-checked,
// so a program built on its acceptance is not a checked program. The checker
// on the lowered form ("the route") therefore decides every product
// compilation by itself, before Psi 05 is given the trees:
//
// - The route rejects: the compilation fails with the route's diagnostics.
// - The route cannot judge the program (a construct it does not serve yet, a
//   proof goal whose form its prover has no rule for, or a defect of the
//   route itself): the compilation fails with an error that opens with
//   `NOT_SUPPORTED_YET` and names what is not supported. It is never handed to
//   Psi 05 for a verdict. A missing verdict is a refusal, never an
//   acceptance.
// - The route accepts: Psi 05 and 06 still run, because product selection,
//   the provider inventory and target preparation read their output until
//   the route's own published module feeds them (TASKS.md,
//   LOWERED-FORM-CHECKER, step 1). They also still judge the two sources the
//   route does not lower, the Build program and the bound target's
//   entry-contract source (`lowered-route-governance-scope`), and a rejection
//   placed there stands. A rejection placed anywhere else is not a verdict on
//   the program, which the route accepted: the compilation fails with
//   `NO_PRODUCT_ROUTE`, because no product can be built without that output.
//
// A compilation that stops before Psi 04 completes has no trees to judge and
// fails on that stage's own diagnostics.

use diagnostics::Diagnostic;
use lowered_discharge::{CompilationRefusal, RouteVerdict};
use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceId, SourceMap};
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
    pub(super) fn judge(&self, typed: &typed_trees::TypedTrees) -> RouteJudgment {
        RouteJudgment::all(
            lowered_discharge::judge_compilation(
                typed,
                structure_elaboration::Compilation {
                    entries: self.entries,
                    subject: self.subject,
                    selected: self.selected,
                    bindings: self.bindings,
                },
            )
            .into_iter()
            .map(RouteJudgment::of)
            .collect(),
        )
    }
}

/// What Psi 05 still judges after the route accepts: the Build program and
/// the bound targets' entry-contract sources, which the route does not lower.
pub(super) struct CurrentScope<'a> {
    pub(super) sources: Option<&'a SourceMap>,
    /// The source file that declares the selected Build machine.
    pub(super) build_program: Option<SourceId>,
    /// The bound targets' entry-contract sources, by package-relative path.
    pub(super) entry_contracts: &'a [target::ProgramEntryPhysicalContractPackage],
}

/// The first sentence of the error a compilation reports when the route
/// accepts a program that Psi 05 or 06, whose output every product is still
/// built from, cannot process.
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

/// The compilation's outcome from `current`, what the check returned, given
/// whether the route `accepted` the program. The check returns the route's own
/// diagnostics when the route does not accept (`RouteJudgment::verdict`), and
/// an earlier stage's when no trees reached the route.
pub(super) fn govern<T>(
    current: Result<T, Vec<Diagnostic>>,
    accepted: bool,
    scope: &CurrentScope<'_>,
) -> Result<T, Vec<Diagnostic>> {
    if !accepted {
        // Nothing may be built without the route's acceptance. A check that
        // returns a program the route never accepted is a defect here.
        return match current {
            Err(rejection) => Err(rejection),
            Ok(_) => Err(vec![route_defect("the check finished without its verdict")]),
        };
    }
    current.map_err(|rejection| {
        if !rejection
            .iter()
            .all(|diagnostic| outside_current_scope(scope, diagnostic))
        {
            return rejection;
        }
        let mut reported = Vec::with_capacity(rejection.len() + 1);
        reported.push(Diagnostic::error(format!(
            "{NO_PRODUCT_ROUTE}: the checker on the lowered form accepts this program, and its \
             verdict is the compilation's; no product can be built for it, because product \
             selection, the provider inventory and target preparation still read the output of \
             the stages being removed, which could not process it; their diagnostics follow"
        )));
        reported.extend(rejection);
        reported
    })
}

/// Whether `diagnostic` is placed in a source the route judges: neither the
/// Build program nor an entry-contract source.
fn outside_current_scope(scope: &CurrentScope<'_>, diagnostic: &Diagnostic) -> bool {
    let (Some(sources), Some(span)) = (scope.sources, diagnostic.source_span) else {
        return false;
    };
    let Some(file) = sources.file_at(span) else {
        return false;
    };
    if scope.build_program == Some(file.source_id) {
        return false;
    }
    let relative = file.path.strip_prefix(&file.package_root).ok();
    !scope
        .entry_contracts
        .iter()
        .any(|package| relative == Some(std::path::Path::new(package.package_relative_source())))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: CurrentScope<'static> = CurrentScope {
        sources: None,
        build_program: None,
        entry_contracts: &[],
    };

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
            },
        ] {
            let reported = messages(RouteJudgment::of(Ok(verdict)).verdict());
            assert!(reported[0].starts_with(NOT_SUPPORTED_YET), "{reported:#?}");
        }
    }

    /// The route accepts and the later stages finish: the program builds.
    #[test]
    fn an_accepted_program_builds() {
        assert!(govern(Ok(()), true, &NONE).is_ok());
    }

    /// Without the route's acceptance nothing builds: the check's own
    /// diagnostics stand, and a check that somehow returned a program is
    /// refused.
    #[test]
    fn nothing_builds_without_the_routes_acceptance() {
        assert_eq!(
            messages(govern(Err(vec![Diagnostic::error("route")]), false, &NONE)),
            ["route"]
        );
        let reported = messages(govern(Ok(()), false, &NONE));
        assert!(reported[0].starts_with(NOT_SUPPORTED_YET), "{reported:#?}");
    }

    /// After the route accepts, a rejection that cannot be placed outside the
    /// Build program and the entry contract is reported as it is.
    #[test]
    fn an_unplaced_later_rejection_stands() {
        assert_eq!(
            messages(govern(Err(vec![Diagnostic::error("current")]), true, &NONE)),
            ["current"]
        );
    }
}

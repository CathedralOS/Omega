// Which checker's verdict a Product compilation reports while both ship.
//
// The owner ruled on 2026-11-12 (`lowered-route-governance-scope`,
// wiki/drafts/designs/check_on_lowered_form.md, "The gate"): the current
// checker judges the Build program and the bound target's entry-contract
// source, the checker on the lowered form judges everything else in the same
// compilation, and a compilation routes on the new verdict only when both
// pass. So the route's verdict on its own scope decides the compilation
// whenever the current checker finds nothing wrong in the Build program or
// the entry contract:
//
// - The route accepts and the current checker accepts: the program builds.
// - The route rejects: the compilation fails with the route's diagnostics,
//   whatever the current checker said about the rest. When the current
//   checker rejects too, its diagnostics follow the route's.
// - The route accepts and the current checker rejects, every rejection lying
//   outside the Build program and the entry contract: the program is valid,
//   but no product can be built for it yet. Terminal Psi and native
//   production run on the current checker's output, which a rejection never
//   produces, and the route's own Terminal module has no production path. The
//   compilation fails with an error that says so, with the current checker's
//   diagnostics after it.
//
// The current checker's verdict stands instead when the route does not
// serve some construct of the closure (`RouteVerdict::FallBack`, the
// design's "falling back to Psi 05 and 06 on any
// `RefusalReason::Unsupported`"), when its prover has no rule for some goal's
// form and nothing else is wrong (`RouteVerdict::Incomplete`: the route
// judges the program neither way, and the closed `Incomplete` outcome has no
// provision for a proof goal form yet, so this is the same standing as an
// unserved construct), when the route itself is defective (an identity that
// does not resolve, which the route returns as `Err`; a router "must not
// accept the program on it"), and when Psi 04 did not complete, so there
// are no trees to judge.
//
// A rejection the current checker places in the Build program or an
// entry-contract source is its own to decide, and stands. A diagnostic with
// no source file cannot be placed, so it counts as one of those.

use diagnostics::Diagnostic;
use lowered_discharge::RouteVerdict;
use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceId, SourceMap};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;

/// What the route judges: the declarations the request claims, which are
/// the same `structure_elaboration::elaborate` roots.
pub(super) struct RouteScope<'a> {
    pub(super) entries: &'a [SymbolHandle],
    pub(super) subject: Option<PackageKeyIdentity>,
    pub(super) selected: &'a [SymbolHandle],
}

impl RouteScope<'_> {
    /// The route's judgment of `typed`, the trees Psi 05 judges: the closure
    /// of the compilation's roots, and the check form of each generic
    /// machine the subject declares. generics.md, "Static type equality":
    /// "An unspecialized body checks every admitted alternative; one
    /// consumer cannot authorize an otherwise invalid generic body." So a
    /// generic body is judged as written whether or not the program applies
    /// it, and the machines stage 04 generated for the program's own
    /// applications are judged as closed machines of the closure. The route
    /// decides only when it decides every one of them.
    pub(super) fn judge(&self, typed: &typed_trees::TypedTrees) -> RouteJudgment {
        let elaborated = structure_elaboration::elaborate(
            typed,
            structure_elaboration::Compilation {
                entries: self.entries,
                subject: self.subject,
                selected: self.selected,
            },
        );
        let templates = match &elaborated {
            Ok(elaboration) => elaboration.generic_templates.clone(),
            Err(_) => Vec::new(),
        };
        let mut judgments = vec![RouteJudgment::of(lowered_discharge::judge(elaborated))];
        if !templates.is_empty() {
            match structure_elaboration::elaborate_generic_forms(typed, &templates) {
                Ok(forms) => judgments.extend(
                    forms
                        .into_iter()
                        .map(|form| RouteJudgment::of(lowered_discharge::judge(Ok(form)))),
                ),
                Err(refused) => {
                    judgments.push(RouteJudgment::of(lowered_discharge::judge(Err(refused))))
                }
            }
        }
        RouteJudgment::all(judgments)
    }
}

/// What the current checker judges besides its verdict on the rest: the
/// Build program and the bound targets' entry-contract sources.
pub(super) struct CurrentScope<'a> {
    pub(super) sources: Option<&'a SourceMap>,
    /// The source file that declares the selected Build machine.
    pub(super) build_program: Option<SourceId>,
    /// The bound targets' entry-contract sources, by package-relative path.
    pub(super) entry_contracts: &'a [target::ProgramEntryPhysicalContractPackage],
}

/// The first sentence of the error a compilation reports when the route
/// accepts a program the current checker rejects outside its own scope.
pub(crate) const NO_PRODUCT_ROUTE: &str = "no product route yet";

/// What the route decided, reduced to what governance reads.
#[derive(Debug)]
pub(super) enum RouteJudgment {
    /// Every obligation of the closure was discharged.
    Accepted,
    /// The program is rejected, with these diagnostics. Never empty.
    Rejected(Vec<Diagnostic>),
    /// The route judges the program neither way: an unserved construct, a
    /// goal form the prover has no rule for, or a defect of the route.
    Undecided,
}

impl RouteJudgment {
    /// The judgment of a compilation made of several separately judged
    /// parts: undecided when any part is, rejected with every part's
    /// diagnostics when any part is rejected, accepted when all are. An
    /// undecided part leaves the whole to the current checker, as an
    /// unserved construct inside one closure does.
    pub(super) fn all(parts: Vec<Self>) -> Self {
        let mut rejected: Option<Vec<Diagnostic>> = None;
        for part in parts {
            match part {
                Self::Undecided => return Self::Undecided,
                Self::Rejected(diagnostics) => {
                    rejected.get_or_insert_with(Vec::new).extend(diagnostics);
                }
                Self::Accepted => {}
            }
        }
        rejected.map_or(Self::Accepted, Self::Rejected)
    }

    pub(super) fn of(verdict: Checked<RouteVerdict>) -> Self {
        let Ok(verdict) = verdict else {
            return Self::Undecided;
        };
        match verdict {
            RouteVerdict::Accepted(_) => Self::Accepted,
            RouteVerdict::FallBack(_) | RouteVerdict::Incomplete(_) => Self::Undecided,
            RouteVerdict::Rejected {
                unproved,
                unestablished,
            } => {
                let mut reported = Vec::new();
                // A rejection may list goals of a form the prover has no
                // rule for beside the ones it failed; those are no claim
                // about the program and are left out.
                for entry in unproved.iter().filter(|entry| !entry.cause.is_incomplete()) {
                    reported.push(entry.reject_diagnostic());
                    reported.extend(entry.opening_diagnostic());
                }
                reported.extend(unestablished.iter().map(|entry| entry.reject_diagnostic()));
                if reported.is_empty() {
                    // `Rejected` is never empty; one that is would be a
                    // defect of the route, and a defect decides nothing.
                    return Self::Undecided;
                }
                Self::Rejected(reported)
            }
        }
    }
}

/// The compilation's outcome under the ruling, from the current checker's
/// `current` and the route's `route` (`None` when no trees reached Psi 05).
pub(super) fn govern<T>(
    current: Result<T, Vec<Diagnostic>>,
    route: Option<RouteJudgment>,
    scope: &CurrentScope<'_>,
) -> Result<T, Vec<Diagnostic>> {
    let Some(route) = route else {
        return current;
    };
    if let Err(rejection) = &current
        && !rejection
            .iter()
            .all(|diagnostic| outside_current_scope(scope, diagnostic))
    {
        return current;
    }
    match route {
        RouteJudgment::Undecided => current,
        // The route's rejection is the compilation's. When the current
        // checker rejects too, the verdict is the same and its diagnostics
        // follow the route's: the route does not yet state every failing
        // condition in the current checker's words (route_one_diagnostics.md),
        // and dropping a correct message would cost the reader.
        RouteJudgment::Rejected(mut reported) => {
            if let Err(rejection) = current {
                reported.extend(rejection);
            }
            Err(reported)
        }
        RouteJudgment::Accepted => current.map_err(|rejection| {
            let mut reported = Vec::with_capacity(rejection.len() + 1);
            reported.push(Diagnostic::error(format!(
                "{NO_PRODUCT_ROUTE}: the checker on the lowered form accepts this program, and \
                 its verdict governs this compilation because every rejection below lies \
                 outside the Build program and the target's entry contract; no product can be \
                 built for it, because Terminal Psi and native production run on the current \
                 checker's output and the lowered form's own module has no production path; the \
                 current checker's diagnostics follow"
            )));
            reported.extend(rejection);
            reported
        }),
    }
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

    fn messages(outcome: Result<(), Vec<Diagnostic>>) -> Vec<String> {
        outcome
            .expect_err("the compilation fails")
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect()
    }

    /// A compilation judged in parts is undecided when any part is, rejected
    /// with every rejected part's diagnostics, and accepted otherwise.
    #[test]
    fn parts_combine() {
        assert!(matches!(
            RouteJudgment::all(vec![RouteJudgment::Accepted, RouteJudgment::Accepted]),
            RouteJudgment::Accepted
        ));
        assert!(matches!(
            RouteJudgment::all(vec![rejected("a"), RouteJudgment::Undecided]),
            RouteJudgment::Undecided
        ));
        let RouteJudgment::Rejected(both) =
            RouteJudgment::all(vec![rejected("a"), RouteJudgment::Accepted, rejected("b")])
        else {
            panic!("rejected");
        };
        assert_eq!(
            both.iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
    }

    /// Both accept: the program builds from the current checker's output.
    #[test]
    fn both_accepting_builds() {
        assert!(govern(Ok(()), Some(RouteJudgment::Accepted), &NONE).is_ok());
    }

    /// The route rejects a program the current checker accepts: the route's
    /// verdict governs and its diagnostics are the compilation's. When the
    /// current checker rejects too, outside its own scope, its diagnostics
    /// follow.
    #[test]
    fn the_routes_rejection_governs_an_accepted_program() {
        assert_eq!(
            messages(govern(Ok(()), Some(rejected("route")), &NONE)),
            ["route"]
        );
    }

    /// The route cannot judge, or there were no trees: the current verdict
    /// stands, whichever it is.
    #[test]
    fn an_undecided_route_leaves_the_current_verdict() {
        assert!(govern(Ok(()), Some(RouteJudgment::Undecided), &NONE).is_ok());
        assert!(govern(Ok(()), None, &NONE).is_ok());
        assert_eq!(
            messages(govern(
                Err(vec![Diagnostic::error("current")]),
                Some(RouteJudgment::Undecided),
                &NONE
            )),
            ["current"]
        );
    }

    /// A current rejection that cannot be placed outside the Build program
    /// and the entry contract is the current checker's to decide: neither
    /// the route's acceptance nor its rejection replaces it.
    #[test]
    fn an_unplaced_current_rejection_stands() {
        for route in [RouteJudgment::Accepted, rejected("route")] {
            assert_eq!(
                messages(govern(
                    Err(vec![Diagnostic::error("current")]),
                    Some(route),
                    &NONE
                )),
                ["current"]
            );
        }
    }
}

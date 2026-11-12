// Which checker's verdict a Product compilation reports while both ship.
//
// The owner ruled on 2026-11-12 (`lowered-route-governance-scope`,
// wiki/drafts/designs/check_on_lowered_form.md, "The gate"): the current
// checker judges the Build program and the bound target's entry-contract
// source, the checker on the lowered form judges everything else in the same
// compilation, and a compilation routes on the new verdict only when both
// accept. Otherwise it keeps the current checker's verdict.
//
// When the current checker accepts, both verdicts end the same way, so the
// route is consulted only after Psi 05 has rejected. It governs that
// compilation when every rejection lies outside the Build program and the
// entry-contract sources (a diagnostic with no source file cannot be placed,
// so it keeps the current verdict) and the route accepts what it judges.
//
// No product can be built for such a compilation yet. Checking reports,
// Terminal Psi and native production all run on the current checker's
// output, which a rejection never produces, and the route's own Terminal
// module has no production path. So a governed compilation still fails, with
// an error that says so and the current checker's diagnostics after it: the
// route changes what the failure reports, never whether a program builds.

use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceId, SourceMap};
use symbols::SymbolHandle;

/// What the route needs from the compilation the current checker rejected.
pub(super) struct RejectedCompilation<'a> {
    /// The typed trees Psi 05 was given.
    pub(super) typed: &'a typed_trees::TypedTrees,
    pub(super) sources: Option<&'a SourceMap>,
    pub(super) entries: &'a [SymbolHandle],
    pub(super) subject: Option<PackageKeyIdentity>,
    pub(super) selected: &'a [SymbolHandle],
    /// The source file that declares the selected Build machine.
    pub(super) build_program: Option<SourceId>,
    /// The bound targets' entry-contract sources, by package-relative path.
    pub(super) entry_contracts: &'a [target::ProgramEntryPhysicalContractPackage],
}

/// The diagnostics the compilation reports in place of `rejection`, or
/// `None` when the current checker's verdict stands.
pub(super) fn governed_rejection(
    compilation: &RejectedCompilation<'_>,
    rejection: &[Diagnostic],
) -> Option<Vec<Diagnostic>> {
    if rejection.is_empty()
        || !rejection
            .iter()
            .all(|diagnostic| outside_current_scope(compilation, diagnostic))
    {
        return None;
    }
    let elaborated = structure_elaboration::elaborate(
        compilation.typed,
        structure_elaboration::Compilation {
            entries: compilation.entries,
            subject: compilation.subject,
            selected: compilation.selected,
        },
    );
    // A rejection, an incomplete judgment, a fallback and a defect of the
    // route all leave the current checker's verdict in place.
    let Ok(lowered_discharge::RouteVerdict::Accepted(_)) = lowered_discharge::judge(elaborated)
    else {
        return None;
    };
    let mut reported = Vec::with_capacity(rejection.len() + 1);
    reported.push(Diagnostic::error(
        "the checker on the lowered form accepts this program, and its verdict governs this \
         compilation because every rejection below lies outside the Build program and the \
         target's entry contract; no product exists yet for a program the current checker \
         rejects, because checking reports, Terminal Psi and native production all run on that \
         checker's output; the current checker's diagnostics follow",
    ));
    reported.extend(rejection.iter().cloned());
    Some(reported)
}

/// Whether `diagnostic` is placed in a source the route judges: neither the
/// Build program nor an entry-contract source.
fn outside_current_scope(compilation: &RejectedCompilation<'_>, diagnostic: &Diagnostic) -> bool {
    let (Some(sources), Some(span)) = (compilation.sources, diagnostic.source_span) else {
        return false;
    };
    let Some(file) = sources.file_at(span) else {
        return false;
    };
    if compilation.build_program == Some(file.source_id) {
        return false;
    }
    let relative = file.path.strip_prefix(&file.package_root).ok();
    !compilation
        .entry_contracts
        .iter()
        .any(|package| relative == Some(std::path::Path::new(package.package_relative_source())))
}

//! Final adjudication of requested physical-authority exclusions.
//!
//! A `PhysicalAuthorityClass` selection records that no admitted mechanism of
//! the artifact's executable composition may exercise that D45 class. The
//! semantic walk in `build-evaluation` bounds the entry/call closure; only
//! this stage can see the settled mechanisms, so the physical axis is
//! adjudicated here against the retained mechanism-closure review.
//!
//! The check reuses `review_terminal_authority_closure`'s classification —
//! each leaf's exercised disposition — and never invents a second classifier.
//! It is deliberately independent of the receiver-admission axis: a requested
//! absence holds even when the request carries no permission policy
//! (`permitted` stays `None`), because excluding a class is not admitting a
//! receiver permission. The receipt binds the artifact identity, target,
//! selected-provider closure, and exact mechanism leaves, so a source-free
//! consumer replays the identical verdict from the retained evidence.

use diagnostics::Diagnostic;

/// Adjudicate the physical axis of one retained exclusion union against the
/// mechanism-closure review provider admission retained for this artifact.
///
/// Every exercised class of every closure leaf must stay outside the
/// requested exclusion set; a leaf admitted without a permission policy is
/// adjudicated identically — classification, not receiver permission, answers
/// the absence question. Crash-cause and service exclusions are semantic
/// verdicts established upstream and do not consult mechanisms here.
pub(crate) fn admit_behavior_exclusion_closure(
    exclusions: &build_evaluation::BehaviorExclusions,
    closure_review: &effects::TerminalAuthorityClosureReviewReceipt,
) -> Result<(), Vec<Diagnostic>> {
    if exclusions.physical_authority_classes().is_empty() {
        return Ok(());
    }
    let mut violations = Vec::new();
    for leaf in closure_review.leaves() {
        for &class in leaf.exercised().classes() {
            if exclusions.excludes_physical_authority_class(class) {
                violations.push(format!(
                    "requirement `{}` exercises excluded physical authority class {class:?} through mechanism {:?}",
                    leaf.requirement_identity(),
                    leaf.mechanism(),
                ));
            }
        }
    }
    if violations.is_empty() {
        return Ok(());
    }
    let mut diagnostics = vec![Diagnostic::error(
        "requested behavior exclusions could not be satisfied by the reviewed mechanism closure"
            .to_owned(),
    )];
    diagnostics.extend(violations.into_iter().map(Diagnostic::error));
    Err(diagnostics)
}

#[cfg(test)]
mod tests;

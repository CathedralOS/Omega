//! Only the receiving provider review establishes which physical classes
//! each selected mechanism can exercise. An unclassified leaf cannot satisfy a ban.

use crate::BehaviorExclusions;
use diagnostics::Diagnostic;

impl BehaviorExclusions {
    pub fn validate_physical_authority_closure(
        &self,
        closure_review: &terminal_authority_receipts::TerminalAuthorityClosureReviewReceipt,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.physical_authority_classes().is_empty() {
            return Ok(());
        }
        let mut violations = Vec::new();
        for leaf in closure_review.leaves() {
            let Some(exercised) = leaf.exercised() else {
                violations.push(format!(
                    "authority subject `{}` has no physical authority classification for mechanism {:?}; requested exclusions cannot be established",
                    closure_review
                        .identities()
                        .text(leaf.subject_identity())
                        .unwrap_or_default(),
                    leaf.mechanism(),
                ));
                continue;
            };
            for &class in exercised.classes() {
                if self.excludes_physical_authority_class(class) {
                    violations.push(format!(
                        "authority subject `{}` exercises excluded physical authority class {class:?} through mechanism {:?}",
                        closure_review
                            .identities()
                            .text(leaf.subject_identity())
                            .unwrap_or_default(),
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
}

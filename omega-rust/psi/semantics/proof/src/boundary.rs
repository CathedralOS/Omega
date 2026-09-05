//! Boundary proof obligations for host/core primitive implementations.
//!
//! Host APIs, inline assembly, target-specific primitives, and core operator
//! implementations sit at the edge of Omega's semantic world. The compiler
//! cannot honestly *derive* their behavior from Omega code, so a boundary
//! provider must instead *declare* what it establishes and what it preserves
//! (see `wiki/language_guide/chapter_9_proof_obligations.md`, "Boundary").
//!
//! This module models the proof-obligation side of that contract only: what a
//! boundary provider must establish (its guarantees) and what invariants it
//! must preserve across the boundary. The registry record that maps a concrete
//! primitive symbol to its provider is another lane's concern; here we only
//! describe the obligations the prover must account for.

use typed_trees::name::Identifier;

/// Why a primitive crosses a boundary instead of being proven from Omega code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoundaryKind {
    /// A host/platform API call (the most common boundary).
    #[default]
    Host,
    /// A core operator/primitive lowered to compiler/runtime machinery below
    /// the public core surface (for example `Slice` indexing).
    CorePrimitive,
    /// Inline assembly or a target-specific intrinsic.
    TargetIntrinsic,
}

impl BoundaryKind {
    /// Stable label for diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            BoundaryKind::Host => "host boundary",
            BoundaryKind::CorePrimitive => "core primitive boundary",
            BoundaryKind::TargetIntrinsic => "target intrinsic boundary",
        }
    }
}

/// What a boundary obligation does with a fact: the provider must either
/// *establish* it as a fresh guarantee, or *preserve* an invariant it received.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoundaryObligationMode {
    /// The provider must establish this fact as a guarantee on exit. Because the
    /// fact cannot be proven from Omega code, the boundary is the accepted
    /// authority for it (chapter 9, "Boundary").
    #[default]
    Establish,
    /// The provider must preserve this invariant across the boundary: it holds
    /// on entry and must still hold on exit.
    Preserve,
}

impl BoundaryObligationMode {
    /// Stable label for diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            BoundaryObligationMode::Establish => "establish",
            BoundaryObligationMode::Preserve => "preserve",
        }
    }
}

/// A single proof obligation a boundary provider carries: a named fact the
/// provider must either establish or preserve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryProofObligation {
    /// The boundary providing this guarantee.
    pub kind: BoundaryKind,
    /// Whether the provider establishes a fresh fact or preserves an invariant.
    pub mode: BoundaryObligationMode,
    /// The fact the boundary is accountable for (a domain, contract, or
    /// invariant name).
    pub fact: Identifier,
    /// The primitive operation the obligation is attached to, for diagnostics.
    pub operation: Identifier,
}

impl BoundaryProofObligation {
    /// A guarantee the boundary establishes on exit.
    pub fn establishes(kind: BoundaryKind, operation: Identifier, fact: Identifier) -> Self {
        Self {
            kind,
            mode: BoundaryObligationMode::Establish,
            fact,
            operation,
        }
    }

    /// An invariant the boundary must preserve across the call.
    pub fn preserves(kind: BoundaryKind, operation: Identifier, fact: Identifier) -> Self {
        Self {
            kind,
            mode: BoundaryObligationMode::Preserve,
            fact,
            operation,
        }
    }

    /// Human-readable description, used in "needs fact X here" style diagnostics.
    pub fn describe(&self) -> String {
        format!(
            "{} must {} `{}` for `{}`",
            self.kind.label(),
            self.mode.label(),
            self.fact,
            self.operation
        )
    }
}

/// The full set of proof obligations a boundary provider must discharge for one
/// primitive implementation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoundaryObligationSet {
    pub obligations: Vec<BoundaryProofObligation>,
}

impl BoundaryObligationSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, obligation: BoundaryProofObligation) {
        self.obligations.push(obligation);
    }

    /// Facts the provider must establish (its guarantees).
    pub fn established(&self) -> impl Iterator<Item = &BoundaryProofObligation> {
        self.obligations
            .iter()
            .filter(|obligation| obligation.mode == BoundaryObligationMode::Establish)
    }

    /// Invariants the provider must preserve across the boundary.
    pub fn preserved(&self) -> impl Iterator<Item = &BoundaryProofObligation> {
        self.obligations
            .iter()
            .filter(|obligation| obligation.mode == BoundaryObligationMode::Preserve)
    }

    pub fn is_empty(&self) -> bool {
        self.obligations.is_empty()
    }

    /// Check this boundary's obligations against the facts currently established
    /// at the boundary site. Returns the obligations that are *not* discharged:
    ///
    /// - an `Establish` obligation is discharged trivially (the boundary is the
    ///   authority for it), so it is never reported here;
    /// - a `Preserve` obligation requires the invariant to already hold on entry
    ///   (`available` must contain its fact) and is reported when it does not.
    ///
    /// Reporting only `Preserve` failures keeps the diagnostic honest: the
    /// compiler cannot derive a boundary's guarantees, but it *can* insist that
    /// an invariant the boundary promises to preserve was actually present.
    pub fn unmet_preservations<'a>(
        &'a self,
        available: &'a [Identifier],
    ) -> impl Iterator<Item = &'a BoundaryProofObligation> + 'a {
        self.preserved()
            .filter(move |obligation| !available.contains(&obligation.fact))
    }

    /// Clear "needs fact X here" diagnostics for every boundary invariant that
    /// must be preserved but is not established at the call site. Empty when all
    /// preserved invariants hold (or the boundary only establishes facts).
    pub fn missing_fact_diagnostics(&self, available: &[Identifier]) -> Vec<String> {
        self.unmet_preservations(available)
            .map(|obligation| {
                format!(
                    "{}: needs fact `{}` established before the call so the {} can \
                     preserve it across `{}`",
                    obligation.describe(),
                    obligation.fact,
                    obligation.kind.label(),
                    obligation.operation
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ident(name: &str) -> Identifier {
        Identifier::generated(name)
    }

    #[test]
    fn establishes_and_preserves_partition_the_set() {
        let mut set = BoundaryObligationSet::new();
        set.push(BoundaryProofObligation::establishes(
            BoundaryKind::CorePrimitive,
            ident("Slice::index"),
            ident("InBounds"),
        ));
        set.push(BoundaryProofObligation::preserves(
            BoundaryKind::CorePrimitive,
            ident("Slice::index"),
            ident("Slice::Length"),
        ));

        assert_eq!(set.established().count(), 1);
        assert_eq!(set.preserved().count(), 1);
        assert!(!set.is_empty());
    }

    #[test]
    fn describe_mentions_mode_and_fact() {
        let obligation = BoundaryProofObligation::establishes(
            BoundaryKind::Host,
            ident("write_all"),
            ident("String::Utf8"),
        );
        let described = obligation.describe();
        assert!(described.contains("establish"));
        assert!(described.contains("String::Utf8"));
        assert!(described.contains("write_all"));
        assert!(described.contains("host boundary"));
    }

    #[test]
    fn preserve_obligation_reports_missing_fact() {
        let mut set = BoundaryObligationSet::new();
        set.push(BoundaryProofObligation::establishes(
            BoundaryKind::CorePrimitive,
            ident("Slice::index"),
            ident("InBounds"),
        ));
        set.push(BoundaryProofObligation::preserves(
            BoundaryKind::CorePrimitive,
            ident("Slice::index"),
            ident("Slice::Length"),
        ));

        // Nothing established: the preserved invariant is reported, the
        // established guarantee is not.
        let diagnostics = set.missing_fact_diagnostics(&[]);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].contains("Slice::Length"));
        assert!(diagnostics[0].contains("needs fact"));

        // Once the invariant is available, no diagnostic is produced.
        assert!(
            set.missing_fact_diagnostics(&[ident("Slice::Length")])
                .is_empty()
        );
    }

    #[test]
    fn establish_only_boundary_never_reports_missing_facts() {
        let mut set = BoundaryObligationSet::new();
        set.push(BoundaryProofObligation::establishes(
            BoundaryKind::Host,
            ident("from_utf8"),
            ident("String::Utf8"),
        ));
        assert!(set.missing_fact_diagnostics(&[]).is_empty());
        assert_eq!(set.unmet_preservations(&[]).count(), 0);
    }

    #[test]
    fn default_mode_is_establish() {
        assert_eq!(
            BoundaryObligationMode::default(),
            BoundaryObligationMode::Establish
        );
        assert_eq!(BoundaryKind::default(), BoundaryKind::Host);
    }
}

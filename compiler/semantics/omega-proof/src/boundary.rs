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

use omega_typed_trees::name::Identifier;

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
    pub fn establishes(
        kind: BoundaryKind,
        operation: Identifier,
        fact: Identifier,
    ) -> Self {
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
    fn default_mode_is_establish() {
        assert_eq!(
            BoundaryObligationMode::default(),
            BoundaryObligationMode::Establish
        );
        assert_eq!(BoundaryKind::default(), BoundaryKind::Host);
    }
}
